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
//! All extractors return **422 Unprocessable Entity** on validation failure.
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

use std::sync::OnceLock;

use salvo::extract::metadata::{Metadata, Source, SourceFrom, SourceParser};
use salvo::extract::Extractible;
use salvo::http::StatusCode;
use salvo::prelude::*;
use vld::schema::VldParse;
use vld_http_common::coerce_value;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error type for vld validation failures in Salvo handlers.
///
/// Implements [`Writer`] so it can be returned from `#[handler]` functions
/// via `Result<T, VldSalvoError>`.
///
/// On write, renders a `422 Unprocessable Entity` JSON response using
/// [`vld_http_common::format_vld_error`].
#[derive(Debug)]
pub struct VldSalvoError {
    /// The underlying validation error.
    pub error: vld::error::VldError,
}

impl VldSalvoError {
    /// Create a new `VldSalvoError` from a [`VldError`](vld::error::VldError).
    pub fn new(error: vld::error::VldError) -> Self {
        Self { error }
    }
}

impl std::fmt::Display for VldSalvoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Validation failed: {}", self.error)
    }
}

impl std::error::Error for VldSalvoError {}

impl From<vld::error::VldError> for VldSalvoError {
    fn from(error: vld::error::VldError) -> Self {
        Self { error }
    }
}

#[async_trait]
impl Writer for VldSalvoError {
    async fn write(mut self, _req: &mut Request, _depot: &mut Depot, res: &mut Response) {
        let body = vld_http_common::format_vld_error(&self.error);
        res.status_code(StatusCode::UNPROCESSABLE_ENTITY);
        res.render(Json(body));
    }
}

// ---------------------------------------------------------------------------
// Helper: build a parse-error VldSalvoError
// ---------------------------------------------------------------------------

fn parse_error(msg: impl std::fmt::Display) -> VldSalvoError {
    VldSalvoError {
        error: vld::error::VldError::single(vld::error::IssueCode::ParseError, msg.to_string()),
    }
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

impl<T> std::ops::Deref for VldJson<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> std::ops::DerefMut for VldJson<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<'ex, T: VldParse + Send> Extractible<'ex> for VldJson<T> {
    fn metadata() -> &'static Metadata {
        static META: OnceLock<Metadata> = OnceLock::new();
        META.get_or_init(|| {
            Metadata::new("VldJson")
                .add_default_source(Source::new(SourceFrom::Body, SourceParser::Json))
        })
    }

    async fn extract(
        req: &'ex mut Request,
        _depot: &'ex mut Depot,
    ) -> Result<Self, impl Writer + Send + std::fmt::Debug + 'static> {
        let value: serde_json::Value = req
            .parse_json()
            .await
            .map_err(|e| parse_error(format_args!("Invalid JSON: {e}")))?;
        T::vld_parse_value(&value)
            .map(VldJson)
            .map_err(VldSalvoError::from)
    }
}

// ============================= VldQuery ======================================

/// Salvo extractor that validates **URL query parameters**.
///
/// Values are coerced: `"42"` → number, `"true"`/`"false"` → boolean,
/// empty → null.
///
/// ```rust,ignore
/// #[handler]
/// async fn search(q: VldQuery<SearchParams>, res: &mut Response) {
///     // q.page, q.limit, ...
/// }
/// ```
pub struct VldQuery<T>(pub T);

impl<T> std::ops::Deref for VldQuery<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> std::ops::DerefMut for VldQuery<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<'ex, T: VldParse + Send> Extractible<'ex> for VldQuery<T> {
    fn metadata() -> &'static Metadata {
        static META: OnceLock<Metadata> = OnceLock::new();
        META.get_or_init(|| {
            Metadata::new("VldQuery")
                .add_default_source(Source::new(SourceFrom::Query, SourceParser::MultiMap))
        })
    }

    async fn extract(
        req: &'ex mut Request,
        _depot: &'ex mut Depot,
    ) -> Result<Self, impl Writer + Send + std::fmt::Debug + 'static> {
        let qs = req.uri().query().unwrap_or("");
        let value = vld_http_common::query_string_to_json(qs);
        T::vld_parse_value(&value)
            .map(VldQuery)
            .map_err(VldSalvoError::from)
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

impl<T> std::ops::Deref for VldForm<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> std::ops::DerefMut for VldForm<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<'ex, T: VldParse + Send> Extractible<'ex> for VldForm<T> {
    fn metadata() -> &'static Metadata {
        static META: OnceLock<Metadata> = OnceLock::new();
        META.get_or_init(|| {
            Metadata::new("VldForm")
                .add_default_source(Source::new(SourceFrom::Body, SourceParser::MultiMap))
        })
    }

    async fn extract(
        req: &'ex mut Request,
        _depot: &'ex mut Depot,
    ) -> Result<Self, impl Writer + Send + std::fmt::Debug + 'static> {
        let body_str = req
            .parse_body::<String>()
            .await
            .map_err(|e| parse_error(format_args!("Invalid form body: {e}")))?;
        let map = vld_http_common::parse_query_string(&body_str);
        let value = serde_json::Value::Object(map);
        T::vld_parse_value(&value)
            .map(VldForm)
            .map_err(VldSalvoError::from)
    }
}

// ============================= VldPath =======================================

/// Salvo extractor that validates **path parameters**.
///
/// Path segment values are coerced (numbers, booleans, etc.).
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

impl<T> std::ops::Deref for VldPath<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> std::ops::DerefMut for VldPath<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<'ex, T: VldParse + Send> Extractible<'ex> for VldPath<T> {
    fn metadata() -> &'static Metadata {
        static META: OnceLock<Metadata> = OnceLock::new();
        META.get_or_init(|| {
            Metadata::new("VldPath")
                .add_default_source(Source::new(SourceFrom::Param, SourceParser::MultiMap))
        })
    }

    async fn extract(
        req: &'ex mut Request,
        _depot: &'ex mut Depot,
    ) -> Result<Self, impl Writer + Send + std::fmt::Debug + 'static> {
        let mut map = serde_json::Map::new();
        for (key, value) in req.params().iter() {
            map.insert(key.clone(), coerce_value(value));
        }
        let value = serde_json::Value::Object(map);
        T::vld_parse_value(&value)
            .map(VldPath)
            .map_err(VldSalvoError::from)
    }
}

// ============================= VldHeaders ====================================

/// Salvo extractor that validates **HTTP headers**.
///
/// Header names are normalised to snake_case: `Content-Type` → `content_type`.
/// Values are coerced (numbers, booleans, etc.).
///
/// ```rust,ignore
/// #[handler]
/// async fn handler(h: VldHeaders<AuthHeaders>, res: &mut Response) {
///     // h.authorization
/// }
/// ```
pub struct VldHeaders<T>(pub T);

impl<T> std::ops::Deref for VldHeaders<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> std::ops::DerefMut for VldHeaders<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<'ex, T: VldParse + Send> Extractible<'ex> for VldHeaders<T> {
    fn metadata() -> &'static Metadata {
        static META: OnceLock<Metadata> = OnceLock::new();
        META.get_or_init(|| {
            Metadata::new("VldHeaders")
                .add_default_source(Source::new(SourceFrom::Header, SourceParser::MultiMap))
        })
    }

    async fn extract(
        req: &'ex mut Request,
        _depot: &'ex mut Depot,
    ) -> Result<Self, impl Writer + Send + std::fmt::Debug + 'static> {
        let mut map = serde_json::Map::new();
        for (name, value) in req.headers().iter() {
            let key = name.as_str().to_lowercase().replace('-', "_");
            if let Ok(v) = value.to_str() {
                map.insert(key, coerce_value(v));
            }
        }
        let value = serde_json::Value::Object(map);
        T::vld_parse_value(&value)
            .map(VldHeaders)
            .map_err(VldSalvoError::from)
    }
}

// ============================= VldCookie =====================================

/// Salvo extractor that validates **cookie values** from the `Cookie` header.
///
/// ```rust,ignore
/// #[handler]
/// async fn dashboard(c: VldCookie<SessionCookies>, res: &mut Response) {
///     // c.session_id
/// }
/// ```
pub struct VldCookie<T>(pub T);

impl<T> std::ops::Deref for VldCookie<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> std::ops::DerefMut for VldCookie<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<'ex, T: VldParse + Send> Extractible<'ex> for VldCookie<T> {
    fn metadata() -> &'static Metadata {
        static META: OnceLock<Metadata> = OnceLock::new();
        META.get_or_init(|| {
            Metadata::new("VldCookie")
                .add_default_source(Source::new(SourceFrom::Cookie, SourceParser::MultiMap))
        })
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
        let value = vld_http_common::cookies_to_json(cookie_header);
        T::vld_parse_value(&value)
            .map(VldCookie)
            .map_err(VldSalvoError::from)
    }
}

// ---------------------------------------------------------------------------
// Prelude
// ---------------------------------------------------------------------------

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{VldCookie, VldForm, VldHeaders, VldJson, VldPath, VldQuery, VldSalvoError};
    pub use vld::prelude::*;
}
