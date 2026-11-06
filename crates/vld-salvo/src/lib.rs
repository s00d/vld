//! # vld-salvo — Salvo integration for `vld`
//!
//! Validation extractors for [Salvo](https://salvo.rs) 1.x. Every extractor
//! implements [`Extractible`], so it is a plain `#[handler]` parameter, like
//! Salvo's own `JsonBody` or `PathParam`.
//!
//! | Extractor | Source |
//! |-----------|--------|
//! | [`VldJson<T>`] | JSON request body |
//! | [`VldQuery<T>`] | URL query parameters |
//! | [`VldForm<T>`] | URL-encoded form body |
//! | [`VldPath<T>`] | Path parameters |
//! | [`VldHeaders<T>`] | HTTP headers |
//! | [`VldCookie<T>`] | Cookie values |
//!
//! A failed extraction is written as JSON by [`VldSalvoError`] (see the
//! [error body format](vld_http_common#error-bodies)): **422** for invalid or
//! malformed input, **413** for a body over the limit, **415** when
//! [`VldJson`] gets no JSON `Content-Type` (`application/json` or
//! `application/*+json`) or [`VldForm`] no
//! `application/x-www-form-urlencoded`.
//!
//! # Example
//!
//! ```no_run
//! use salvo::prelude::*;
//! use vld_salvo::prelude::*;
//!
//! vld::schema! {
//!     #[derive(Debug, Clone)]
//!     pub struct CreateUser {
//!         pub name: String  => vld::string().min(2),
//!         pub email: String => vld::string().email(),
//!     }
//! }
//!
//! #[handler]
//! async fn create(body: VldJson<CreateUser>, res: &mut Response) {
//!     res.render(Json(serde_json::json!({ "name": body.name })));
//! }
//!
//! # #[tokio::main]
//! # async fn main() {
//! let router = Router::with_path("users").post(create);
//! let acceptor = TcpListener::new("0.0.0.0:5800").bind().await;
//! Server::new(acceptor).serve(router).await;
//! # }
//! ```
//!
//! # Body limits
//!
//! Bodies are read up to Salvo's [`Request::secure_max_size`], set with the
//! `SecureMaxSize` middleware, `Request::set_secure_max_size` or
//! `salvo::http::request::set_global_secure_max_size`. While it is Salvo's
//! built-in 64 KiB default, [`DEFAULT_BODY_LIMIT`] (2 MiB) applies instead,
//! like in every other `vld` adapter — so setting exactly 64 KiB counts as
//! unset.
//!
//! Requires Rust **1.94+** (Salvo 1.x).
//!
//! Guide: <https://s00d.github.io/vld/integrations/web/salvo>

use salvo::extract::Extractible;
use salvo::extract::metadata::{Metadata, Source, SourceFrom, SourceParser};
use salvo::http::StatusCode;
use salvo::prelude::*;
use std::sync::OnceLock;
use vld::schema::VldParse;
use vld_http_common::{
    DEFAULT_BODY_LIMIT, VldHttpError, cookie_headers_to_json, headers_to_json, parse_coerced,
    parse_form_body, parse_json_body, path_params_to_json, query_string_to_json,
    require_form_content_type, require_json_content_type,
};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error type for vld validation failures in Salvo handlers.
///
/// Implements [`Writer`] so it can be returned from `#[handler]` functions
/// via `Result<T, VldSalvoError>`.
///
/// On write, renders the [`VldHttpError`] it wraps as a JSON response
/// (`422` for validation/parse, `415` for a wrong `Content-Type`, `413` for
/// payload too large).
#[derive(Debug)]
pub struct VldSalvoError(VldHttpError);

impl VldSalvoError {
    /// Create a new `VldSalvoError` from a [`VldError`](vld::error::VldError).
    pub fn new(error: vld::error::VldError) -> Self {
        VldHttpError::validation(error).into()
    }

    /// The underlying validation error.
    pub fn error(&self) -> &vld::error::VldError {
        self.0.error()
    }

    /// HTTP status (422, 413 or 415).
    pub fn status(&self) -> StatusCode {
        StatusCode::from_u16(self.0.status()).unwrap_or(StatusCode::UNPROCESSABLE_ENTITY)
    }

    /// JSON response body.
    pub fn body(&self) -> serde_json::Value {
        self.0.body()
    }
}

impl From<VldHttpError> for VldSalvoError {
    fn from(e: VldHttpError) -> Self {
        Self(e)
    }
}

impl std::fmt::Display for VldSalvoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for VldSalvoError {}

impl From<vld::error::VldError> for VldSalvoError {
    fn from(error: vld::error::VldError) -> Self {
        Self::new(error)
    }
}

#[async_trait]
impl Writer for VldSalvoError {
    async fn write(mut self, _req: &mut Request, _depot: &mut Depot, res: &mut Response) {
        res.status_code(self.status());
        res.render(Json(self.body()));
    }
}

/// Salvo's built-in `secure_max_size` (64 KiB).
const SALVO_DEFAULT_MAX_SIZE: usize = 64 * 1024;

/// Read the body up to `secure_max_size`, or [`DEFAULT_BODY_LIMIT`] while
/// that is Salvo's built-in default.
async fn read_body(req: &mut Request) -> Result<&[u8], VldSalvoError> {
    let limit = match req.secure_max_size() {
        SALVO_DEFAULT_MAX_SIZE => DEFAULT_BODY_LIMIT,
        limit => limit,
    };
    req.payload_with_max_size(limit)
        .await
        .map(|b| &b[..])
        .map_err(map_parse_error)
}

fn content_type(req: &Request) -> Option<&str> {
    req.headers()
        .get(salvo::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
}

fn map_parse_error(e: salvo::http::ParseError) -> VldSalvoError {
    match e {
        salvo::http::ParseError::PayloadTooLarge => VldHttpError::payload_too_large().into(),
        other => VldHttpError::parse(other.to_string()).into(),
    }
}

/// Validate a value coerced from text (query, path, form, headers, cookies).
fn validate<T: VldParse>(value: &serde_json::Value) -> Result<T, VldSalvoError> {
    parse_coerced(value).map_err(VldSalvoError::from)
}

fn metadata(name: &'static str, from: SourceFrom, parser: SourceParser) -> Metadata {
    Metadata::new(name).add_default_source(Source::new(from, parser))
}

// ============================= VldJson =======================================

/// Salvo extractor that validates **JSON request bodies**.
///
/// Use as a `#[handler]` parameter:
///
/// ```rust,ignore
/// #[handler]
/// async fn create(body: VldJson<CreateUser>, res: &mut Response) {
///     // body.0 is the validated CreateUser
///     res.render(Json(body.0));
/// }
/// ```
pub struct VldJson<T>(pub T);

impl<'ex, T: VldParse + Send> Extractible<'ex> for VldJson<T> {
    fn metadata() -> &'static Metadata {
        static META: OnceLock<Metadata> = OnceLock::new();
        META.get_or_init(|| metadata("VldJson", SourceFrom::Body, SourceParser::Json))
    }

    async fn extract(
        req: &'ex mut Request,
        _depot: &'ex mut Depot,
    ) -> Result<Self, impl Writer + Send + std::fmt::Debug + 'static> {
        // Read the raw body: salvo's `parse_json` rejects `application/*+json`.
        require_json_content_type(content_type(req)).map_err(VldSalvoError::from)?;
        let value = parse_json_body(read_body(req).await?).map_err(VldSalvoError::from)?;
        T::vld_parse_value(&value)
            .map(VldJson)
            .map_err(VldSalvoError::from)
    }
}

// ============================= VldQuery ======================================

/// Salvo extractor that validates **URL query parameters**.
///
/// See [`vld_http_common::parse_query_string`] for coercion and array rules
/// (`a=1&a=2` / `a[]=1` → arrays).
///
/// ```rust,ignore
/// #[handler]
/// async fn search(q: VldQuery<SearchParams>, res: &mut Response) {
///     // q.page, q.limit, ...
/// }
/// ```
pub struct VldQuery<T>(pub T);

impl<'ex, T: VldParse + Send> Extractible<'ex> for VldQuery<T> {
    fn metadata() -> &'static Metadata {
        static META: OnceLock<Metadata> = OnceLock::new();
        META.get_or_init(|| metadata("VldQuery", SourceFrom::Query, SourceParser::MultiMap))
    }

    async fn extract(
        req: &'ex mut Request,
        _depot: &'ex mut Depot,
    ) -> Result<Self, impl Writer + Send + std::fmt::Debug + 'static> {
        validate(&query_string_to_json(req.uri().query().unwrap_or(""))).map(VldQuery)
    }
}

// ============================= VldForm =======================================

/// Salvo extractor that validates **URL-encoded form bodies**.
///
/// ```rust,ignore
/// #[handler]
/// async fn login(form: VldForm<LoginForm>, res: &mut Response) {
///     // form.username, form.password
/// }
/// ```
pub struct VldForm<T>(pub T);

impl<'ex, T: VldParse + Send> Extractible<'ex> for VldForm<T> {
    fn metadata() -> &'static Metadata {
        static META: OnceLock<Metadata> = OnceLock::new();
        META.get_or_init(|| metadata("VldForm", SourceFrom::Body, SourceParser::MultiMap))
    }

    async fn extract(
        req: &'ex mut Request,
        _depot: &'ex mut Depot,
    ) -> Result<Self, impl Writer + Send + std::fmt::Debug + 'static> {
        require_form_content_type(content_type(req)).map_err(VldSalvoError::from)?;
        let value = parse_form_body(read_body(req).await?).map_err(VldSalvoError::from)?;
        validate(&value).map(VldForm)
    }
}

// ============================= VldPath =======================================

/// Salvo extractor that validates **path parameters**.
///
/// Path segment values are coerced the same way as query parameters.
///
/// ```rust,ignore
/// // Route: /users/{id}
/// vld::schema! {
///     #[derive(Debug, Clone)]
///     pub struct UserId {
///         pub id: i64 => vld::number().int().min(1),
///     }
/// }
///
/// #[handler]
/// async fn get_user(p: VldPath<UserId>, res: &mut Response) {
///     // p.id
/// }
/// ```
pub struct VldPath<T>(pub T);

impl<'ex, T: VldParse + Send> Extractible<'ex> for VldPath<T> {
    fn metadata() -> &'static Metadata {
        static META: OnceLock<Metadata> = OnceLock::new();
        META.get_or_init(|| metadata("VldPath", SourceFrom::Param, SourceParser::MultiMap))
    }

    async fn extract(
        req: &'ex mut Request,
        _depot: &'ex mut Depot,
    ) -> Result<Self, impl Writer + Send + std::fmt::Debug + 'static> {
        let value = path_params_to_json(req.params().iter().map(|(k, v)| (k.as_str(), v.as_str())));
        validate(&value).map(VldPath)
    }
}

// ============================= VldHeaders ====================================

/// Salvo extractor that validates **HTTP headers**.
///
/// See [`vld_http_common::headers_to_json`]: names become snake_case
/// (`Content-Type` → `content_type`), names containing `_` are ignored,
/// repeated headers are joined with `", "`.
///
/// ```rust,ignore
/// #[handler]
/// async fn handler(h: VldHeaders<AuthHeaders>, res: &mut Response) {
///     // h.authorization
/// }
/// ```
pub struct VldHeaders<T>(pub T);

impl<'ex, T: VldParse + Send> Extractible<'ex> for VldHeaders<T> {
    fn metadata() -> &'static Metadata {
        static META: OnceLock<Metadata> = OnceLock::new();
        META.get_or_init(|| metadata("VldHeaders", SourceFrom::Header, SourceParser::MultiMap))
    }

    async fn extract(
        req: &'ex mut Request,
        _depot: &'ex mut Depot,
    ) -> Result<Self, impl Writer + Send + std::fmt::Debug + 'static> {
        let value = headers_to_json(
            req.headers()
                .iter()
                .map(|(n, v)| (n.as_str(), v.as_bytes())),
        );
        validate(&value).map(VldHeaders)
    }
}

// ============================= VldCookie =====================================

/// Salvo extractor that validates **cookie values** from the `Cookie`
/// headers (all of them; the first occurrence of a name wins).
///
/// Values are unquoted, percent-decoded and coerced.
///
/// ```rust,ignore
/// #[handler]
/// async fn dashboard(c: VldCookie<SessionCookies>, res: &mut Response) {
///     // c.session_id
/// }
/// ```
pub struct VldCookie<T>(pub T);

impl<'ex, T: VldParse + Send> Extractible<'ex> for VldCookie<T> {
    fn metadata() -> &'static Metadata {
        static META: OnceLock<Metadata> = OnceLock::new();
        META.get_or_init(|| metadata("VldCookie", SourceFrom::Cookie, SourceParser::MultiMap))
    }

    async fn extract(
        req: &'ex mut Request,
        _depot: &'ex mut Depot,
    ) -> Result<Self, impl Writer + Send + std::fmt::Debug + 'static> {
        let value = cookie_headers_to_json(
            req.headers()
                .get_all(salvo::http::header::COOKIE)
                .iter()
                .map(|v| v.as_bytes()),
        );
        validate(&value).map(VldCookie)
    }
}

vld_http_common::impl_deref!(VldJson, VldQuery, VldPath, VldForm, VldHeaders, VldCookie);

// ---------------------------------------------------------------------------
// Prelude
// ---------------------------------------------------------------------------

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{VldCookie, VldForm, VldHeaders, VldJson, VldPath, VldQuery, VldSalvoError};
    pub use vld::prelude::*;
}
