//! # vld-axum — Axum integration for the `vld` validation library
//!
//! Provides extractors that validate request data using `vld` schemas:
//!
//! | Extractor | Replaces | Source |
//! |---|---|---|
//! | [`VldJson<T>`] | `axum::Json<T>` | JSON request body |
//! | [`VldQuery<T>`] | `axum::extract::Query<T>` | URL query parameters |
//! | [`VldPath<T>`] | `axum::extract::Path<T>` | URL path parameters |
//! | [`VldForm<T>`] | `axum::extract::Form<T>` | URL-encoded form body |
//! | [`VldHeaders<T>`] | manual header extraction | HTTP headers |
//! | [`VldCookie<T>`] | manual cookie parsing | Cookie values |
//!
//! All extractors return **422 Unprocessable Entity** on validation failure
//! or malformed input, and **413 Payload Too Large** when the body exceeds
//! axum's `DefaultBodyLimit` (2 MiB unless configured with
//! `.layer(DefaultBodyLimit::max(n))`). [`VldJson`] requires a JSON
//! `Content-Type` (`application/json` or `application/*+json`) and
//! [`VldForm`] `application/x-www-form-urlencoded`; anything else is
//! **415 Unsupported Media Type**. See the
//! [error body format](vld_http_common#error-bodies).
//!
//! # Quick example
//!
//! ```ignore
//! use axum::{Router, routing::post};
//! use vld::prelude::*;
//! use vld_axum::{VldPath, VldQuery, VldJson, VldHeaders};
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct PathParams {
//!         pub id: i64 => vld::number().int().min(1),
//!     }
//! }
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct Auth {
//!         pub authorization: String => vld::string().min(1),
//!     }
//! }
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct Body {
//!         pub name: String => vld::string().min(2),
//!     }
//! }
//!
//! async fn handler(
//!     VldPath(path): VldPath<PathParams>,
//!     VldHeaders(headers): VldHeaders<Auth>,
//!     VldJson(body): VldJson<Body>,
//! ) -> String {
//!     format!("id={} auth={} name={}", path.id, headers.authorization, body.name)
//! }
//! ```

use axum::extract::rejection::RawPathParamsRejection;
use axum::extract::{
    FromRequest, FromRequestParts, MatchedPath, OriginalUri, RawPathParams, Request,
};
use axum::response::{IntoResponse, Response};
use http::request::Parts;
use http::{header, StatusCode};
use vld::schema::VldParse;
use vld_http_common::{
    cookie_headers_to_json, headers_to_json, parse_form_body, parse_json_body, path_params_to_json,
    percent_decode, query_string_to_json, require_form_content_type, require_json_content_type,
    VldHttpError,
};

// ============================= Rejection =====================================

/// Rejection type returned when validation or body reading fails.
///
/// Used by all `Vld*` extractors in this crate.
pub struct VldJsonRejection(VldHttpError);

impl VldJsonRejection {
    /// Get a reference to the underlying `VldError`.
    pub fn error(&self) -> &vld::error::VldError {
        self.0.error()
    }

    /// HTTP status of this rejection (422, 413 or 415).
    pub fn status(&self) -> StatusCode {
        StatusCode::from_u16(self.0.status()).unwrap_or(StatusCode::UNPROCESSABLE_ENTITY)
    }

    /// JSON response body.
    pub fn body(&self) -> serde_json::Value {
        self.0.body()
    }
}

impl From<VldHttpError> for VldJsonRejection {
    fn from(e: VldHttpError) -> Self {
        Self(e)
    }
}

impl From<vld::error::VldError> for VldJsonRejection {
    fn from(e: vld::error::VldError) -> Self {
        Self(e.into())
    }
}

impl IntoResponse for VldJsonRejection {
    fn into_response(self) -> Response {
        (
            self.status(),
            [(http::header::CONTENT_TYPE, "application/json")],
            self.0.body().to_string(),
        )
            .into_response()
    }
}

impl std::fmt::Display for VldJsonRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::fmt::Debug for VldJsonRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VldJsonRejection")
            .field("error", self.error())
            .field("status", &self.status())
            .finish()
    }
}

impl std::error::Error for VldJsonRejection {}

/// Read the body via axum's `Bytes` extractor (respects `DefaultBodyLimit`).
async fn read_body<S: Send + Sync>(req: Request, state: &S) -> Result<bytes::Bytes, VldHttpError> {
    <bytes::Bytes as FromRequest<S>>::from_request(req, state)
        .await
        .map_err(|rejection| {
            if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
                VldHttpError::payload_too_large()
            } else {
                VldHttpError::parse(format!("Failed to read request body: {rejection}"))
            }
        })
}

/// Validate a value coerced from text (query, path, form, headers, cookies).
fn validate<T: VldParse>(value: &serde_json::Value) -> Result<T, VldJsonRejection> {
    vld_http_common::parse_coerced(value).map_err(VldJsonRejection::from)
}

fn content_type(headers: &http::HeaderMap) -> Option<&str> {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
}

/// Path params decoded lossily (`%FF` → `U+FFFD`), for when axum refuses
/// them as invalid UTF-8: match the raw path against the route template.
fn lossy_path_params(parts: &Parts) -> serde_json::Value {
    let Some(matched) = parts.extensions.get::<MatchedPath>() else {
        return serde_json::json!({});
    };
    // Nested routers see a stripped URI; the template is the full path.
    let uri = parts
        .extensions
        .get::<OriginalUri>()
        .map_or(&parts.uri, |o| &o.0);
    let mut values = uri.path().split('/');
    let mut params = Vec::new();
    for pattern in matched.as_str().split('/') {
        let value = values.next();
        let Some(name) = pattern.strip_prefix('{').and_then(|p| p.strip_suffix('}')) else {
            continue;
        };
        if let Some(name) = name.strip_prefix('*') {
            let rest: Vec<&str> = value.into_iter().chain(values.by_ref()).collect();
            params.push((name, percent_decode(&rest.join("/"))));
            break;
        }
        if let Some(value) = value {
            params.push((name, percent_decode(value)));
        }
    }
    path_params_to_json(params.iter().map(|(k, v)| (*k, v.as_str())))
}

// ============================= VldJson =======================================

/// Axum extractor that validates **JSON request bodies**.
///
/// Drop-in replacement for `axum::Json<T>`.
pub struct VldJson<T>(pub T);

impl<S, T> FromRequest<S> for VldJson<T>
where
    S: Send + Sync,
    T: VldParse,
{
    type Rejection = VldJsonRejection;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        require_json_content_type(content_type(req.headers()))?;
        let body = read_body(req, state).await?;
        T::vld_parse_value(&parse_json_body(&body)?)
            .map(VldJson)
            .map_err(VldJsonRejection::from)
    }
}

// ============================= VldQuery ======================================

/// Axum extractor that validates **URL query parameters**.
///
/// Drop-in replacement for `axum::extract::Query<T>`.
///
/// See [`vld_http_common::parse_query_string`] for coercion and array rules
/// (`a=1&a=2` / `a[]=1` → arrays; non-canonical numbers stay strings).
pub struct VldQuery<T>(pub T);

impl<S, T> FromRequestParts<S> for VldQuery<T>
where
    S: Send + Sync,
    T: VldParse,
{
    type Rejection = VldJsonRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        validate(&query_string_to_json(parts.uri.query().unwrap_or(""))).map(VldQuery)
    }
}

// ============================= VldPath =======================================

/// Axum extractor that validates **URL path parameters**.
///
/// Drop-in replacement for `axum::extract::Path<T>`.
///
/// Path segment values are percent-decoded (invalid UTF-8 becomes `U+FFFD`)
/// and coerced the same way as query parameters. On a route without
/// parameters the input is an empty object.
///
/// # Example
///
/// ```ignore
/// // Route: /users/{id}/posts/{post_id}
/// vld::schema! {
///     #[derive(Debug)]
///     pub struct PostPath {
///         pub id: i64 => vld::number().int().min(1),
///         pub post_id: i64 => vld::number().int().min(1),
///     }
/// }
///
/// async fn get_post(VldPath(p): VldPath<PostPath>) -> String {
///     format!("user {} post {}", p.id, p.post_id)
/// }
/// ```
pub struct VldPath<T>(pub T);

impl<S, T> FromRequestParts<S> for VldPath<T>
where
    S: Send + Sync,
    T: VldParse,
{
    type Rejection = VldJsonRejection;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let value = match RawPathParams::from_request_parts(parts, state).await {
            Ok(params) => path_params_to_json(&params),
            Err(RawPathParamsRejection::InvalidUtf8InPathParam(_)) => lossy_path_params(parts),
            Err(_) => serde_json::json!({}),
        };
        validate(&value).map(VldPath)
    }
}

// ============================= VldForm =======================================

/// Axum extractor that validates **URL-encoded form bodies**
/// (`application/x-www-form-urlencoded`).
///
/// Drop-in replacement for `axum::extract::Form<T>`.
///
/// Values are parsed the same way as query parameters.
///
/// # Example
///
/// ```ignore
/// vld::schema! {
///     #[derive(Debug)]
///     pub struct LoginForm {
///         pub username: String => vld::string().min(3).max(50),
///         pub password: String => vld::string().min(8),
///     }
/// }
///
/// async fn login(VldForm(form): VldForm<LoginForm>) -> String {
///     format!("Welcome, {}!", form.username)
/// }
/// ```
pub struct VldForm<T>(pub T);

impl<S, T> FromRequest<S> for VldForm<T>
where
    S: Send + Sync,
    T: VldParse,
{
    type Rejection = VldJsonRejection;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        require_form_content_type(content_type(req.headers()))?;
        let body = read_body(req, state).await?;
        validate(&parse_form_body(&body)?).map(VldForm)
    }
}

// ============================= VldHeaders ====================================

/// Axum extractor that validates **HTTP headers**.
///
/// See [`vld_http_common::headers_to_json`]: names become snake_case
/// (`X-Request-Id` → `x_request_id`), names containing `_` are ignored,
/// repeated headers are joined with `", "`, non-UTF-8 bytes are replaced
/// with `U+FFFD`.
///
/// # Example
///
/// ```ignore
/// vld::schema! {
///     #[derive(Debug)]
///     pub struct RequiredHeaders {
///         pub authorization: String => vld::string().min(1),
///         pub x_request_id: Option<String> => vld::string().uuid().optional(),
///     }
/// }
///
/// async fn handler(VldHeaders(h): VldHeaders<RequiredHeaders>) -> String {
///     format!("auth={}", h.authorization)
/// }
/// ```
pub struct VldHeaders<T>(pub T);

impl<S, T> FromRequestParts<S> for VldHeaders<T>
where
    S: Send + Sync,
    T: VldParse,
{
    type Rejection = VldJsonRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let value = headers_to_json(
            parts
                .headers
                .iter()
                .map(|(n, v)| (n.as_str(), v.as_bytes())),
        );
        validate(&value).map(VldHeaders)
    }
}

// ============================= VldCookie =====================================

/// Axum extractor that validates **cookie values** from the `Cookie`
/// headers (all of them; the first occurrence of a name wins).
///
/// Cookie names are used as-is for schema field matching.
/// Values are unquoted, percent-decoded and coerced.
///
/// # Example
///
/// ```ignore
/// vld::schema! {
///     #[derive(Debug)]
///     pub struct SessionCookies {
///         pub session_id: String => vld::string().min(1),
///         pub theme: Option<String> => vld::string().optional(),
///     }
/// }
///
/// async fn dashboard(VldCookie(c): VldCookie<SessionCookies>) -> String {
///     format!("session={}", c.session_id)
/// }
/// ```
pub struct VldCookie<T>(pub T);

impl<S, T> FromRequestParts<S> for VldCookie<T>
where
    S: Send + Sync,
    T: VldParse,
{
    type Rejection = VldJsonRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let value = cookie_headers_to_json(
            parts
                .headers
                .get_all(header::COOKIE)
                .iter()
                .map(|v| v.as_bytes()),
        );
        validate(&value).map(VldCookie)
    }
}

vld_http_common::impl_deref!(VldJson, VldQuery, VldPath, VldForm, VldHeaders, VldCookie);

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{VldCookie, VldForm, VldHeaders, VldJson, VldJsonRejection, VldPath, VldQuery};
    pub use vld::prelude::*;
}
