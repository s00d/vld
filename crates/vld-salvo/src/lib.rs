//! # vld-salvo — Salvo integration for `vld`
//!
//! Validation extractors for [Salvo](https://salvo.rs).
//! All extractors implement [`Extractible`] and can be used directly as
//! `#[handler]` function parameters — just like Salvo's built-in `JsonBody`
//! or `PathParam`.
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
//! All extractors return **422 Unprocessable Entity** on validation failure
//! or malformed input, and **413 Payload Too Large** when the body exceeds
//! the limit. [`VldJson`] requires a JSON `Content-Type` (`application/json`
//! or `application/*+json`) and answers **415 Unsupported Media Type**
//! otherwise.
//!
//! # Body limits
//!
//! Bodies are read with Salvo's [`Request::secure_max_size`] (64 KiB unless
//! raised with the `SecureMaxSize` middleware, `Request::set_secure_max_size`
//! or `salvo::http::request::set_global_secure_max_size`).
//!
//! # Quick Example
//!
//! ```rust,ignore
//! use salvo::prelude::*;
//! use vld_salvo::prelude::*;
//!
//! vld::schema! {
//!     #[derive(Debug, Clone, serde::Serialize)]
//!     pub struct CreateUser {
//!         pub name: String  => vld::string().min(2),
//!         pub email: String => vld::string().email(),
//!     }
//! }
//!
//! // VldJson<T> is used as a handler parameter — no manual extraction needed!
//! #[handler]
//! async fn create(body: VldJson<CreateUser>, res: &mut Response) {
//!     res.render(Json(serde_json::json!({"name": body.name})));
//! }
//! ```

use salvo::extract::metadata::{Metadata, Source, SourceFrom, SourceParser};
use salvo::extract::Extractible;
use salvo::http::StatusCode;
use salvo::prelude::*;
use std::sync::OnceLock;
use vld::schema::VldParse;
use vld_http_common::{
    cookies_to_json, headers_to_json, parse_coerced, parse_form_body, parse_json_body,
    path_params_to_json, query_string_to_json, require_json_content_type, VldHttpError,
};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error type for vld validation failures in Salvo handlers.
///
/// Implements [`Writer`] so it can be returned from `#[handler]` functions
/// via `Result<T, VldSalvoError>`.
///
/// On write, renders a JSON response (`422` for validation/parse, `415` for a
/// non-JSON `Content-Type`, `413` for
/// payload too large) using [`vld_http_common`] helpers.
#[derive(Debug)]
pub struct VldSalvoError {
    /// The underlying validation error.
    pub error: vld::error::VldError,
    status: StatusCode,
}

impl VldSalvoError {
    /// Create a new `VldSalvoError` from a [`VldError`](vld::error::VldError).
    pub fn new(error: vld::error::VldError) -> Self {
        VldHttpError::validation(error).into()
    }
}

impl From<VldHttpError> for VldSalvoError {
    fn from(e: VldHttpError) -> Self {
        Self {
            status: StatusCode::from_u16(e.status()).unwrap_or(StatusCode::UNPROCESSABLE_ENTITY),
            error: e.into_error(),
        }
    }
}

impl std::fmt::Display for VldSalvoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.status {
            StatusCode::PAYLOAD_TOO_LARGE => f.write_str("Payload too large"),
            StatusCode::UNSUPPORTED_MEDIA_TYPE => f.write_str("Unsupported Media Type"),
            _ => write!(f, "Validation failed: {}", self.error),
        }
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
        let body = match self.status {
            StatusCode::PAYLOAD_TOO_LARGE => vld_http_common::format_payload_too_large(),
            StatusCode::UNSUPPORTED_MEDIA_TYPE => {
                vld_http_common::format_unsupported_media_type()
            }
            _ => vld_http_common::format_vld_error(&self.error),
        };
        res.status_code(self.status);
        res.render(Json(body));
    }
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
        require_json_content_type(
            req.headers()
                .get(salvo::http::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
        )
        .map_err(VldSalvoError::from)?;
        let limit = req.secure_max_size();
        let body = req
            .payload_with_max_size(limit)
            .await
            .map_err(map_parse_error)?;
        let value = parse_json_body(body).map_err(VldSalvoError::from)?;
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
        let limit = req.secure_max_size();
        let body = req
            .payload_with_max_size(limit)
            .await
            .map_err(map_parse_error)?;
        let value = parse_form_body(body).map_err(VldSalvoError::from)?;
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
/// (`Content-Type` → `content_type`), repeated headers are joined with `", "`.
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

/// Salvo extractor that validates **cookie values** from the `Cookie` header.
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
        let cookie_header = req
            .headers()
            .get("cookie")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        validate(&cookies_to_json(cookie_header)).map(VldCookie)
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
