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
//! All extractors return **422 Unprocessable Entity** on validation failure.
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

use axum::extract::{FromRequest, FromRequestParts, Request};
use axum::response::{IntoResponse, Response};
use http::request::Parts;
use http::StatusCode;

// ============================= Rejection =====================================

/// Rejection type returned when validation or body reading fails.
///
/// Used by all `Vld*` extractors in this crate.
pub struct VldJsonRejection {
    error: vld::error::VldError,
    status: StatusCode,
}

impl VldJsonRejection {
    fn validation(error: vld::error::VldError) -> Self {
        Self {
            error,
            status: StatusCode::UNPROCESSABLE_ENTITY,
        }
    }

    fn parse(message: impl Into<String>) -> Self {
        Self::validation(vld::error::VldError::single(
            vld::error::IssueCode::ParseError,
            message.into(),
        ))
    }

    fn payload_too_large() -> Self {
        Self {
            error: vld::error::VldError::single(
                vld::error::IssueCode::ParseError,
                "Payload too large",
            ),
            status: StatusCode::PAYLOAD_TOO_LARGE,
        }
    }

    /// Get a reference to the underlying `VldError`.
    pub fn error(&self) -> &vld::error::VldError {
        &self.error
    }
}

impl IntoResponse for VldJsonRejection {
    fn into_response(self) -> Response {
        let body = if self.status == StatusCode::PAYLOAD_TOO_LARGE {
            vld_http_common::format_payload_too_large()
        } else {
            vld_http_common::format_vld_error(&self.error)
        };

        (
            self.status,
            [(http::header::CONTENT_TYPE, "application/json")],
            body.to_string(),
        )
            .into_response()
    }
}

impl std::fmt::Display for VldJsonRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Validation failed: {}", self.error)
    }
}

impl std::fmt::Debug for VldJsonRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VldJsonRejection")
            .field("error", &self.error)
            .field("status", &self.status)
            .finish()
    }
}

// ============================= VldJson =======================================

/// Axum extractor that validates **JSON request bodies**.
///
/// Drop-in replacement for `axum::Json<T>`.
pub struct VldJson<T>(pub T);

impl<S, T> FromRequest<S> for VldJson<T>
where
    S: Send + Sync,
    T: vld::schema::VldParse,
{
    type Rejection = VldJsonRejection;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        // Bytes extractor respects axum `DefaultBodyLimit` (unlike raw to_bytes).
        let body = <axum::body::Bytes as FromRequest<S>>::from_request(req, state)
            .await
            .map_err(|rejection| {
                let resp = rejection.into_response();
                if resp.status() == StatusCode::PAYLOAD_TOO_LARGE {
                    VldJsonRejection::payload_too_large()
                } else {
                    VldJsonRejection::parse("Failed to read request body")
                }
            })?;

        let value: serde_json::Value =
            serde_json::from_slice(&body).map_err(|e| VldJsonRejection::parse(format!("Invalid JSON: {}", e)))?;

        let parsed = T::vld_parse_value(&value).map_err(VldJsonRejection::validation)?;

        Ok(VldJson(parsed))
    }
}

// ============================= VldQuery ======================================

/// Axum extractor that validates **URL query parameters**.
///
/// Drop-in replacement for `axum::extract::Query<T>`.
///
/// Values are coerced: `"42"` → number, `"true"`/`"false"` → boolean, empty → null.
pub struct VldQuery<T>(pub T);

impl<S, T> FromRequestParts<S> for VldQuery<T>
where
    S: Send + Sync,
    T: vld::schema::VldParse,
{
    type Rejection = VldJsonRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let query_string = parts.uri.query().unwrap_or("");
        let value = query_string_to_json(query_string);

        let parsed = T::vld_parse_value(&value).map_err(VldJsonRejection::validation)?;

        Ok(VldQuery(parsed))
    }
}

// ============================= VldPath =======================================

/// Axum extractor that validates **URL path parameters**.
///
/// Drop-in replacement for `axum::extract::Path<T>`.
///
/// Path segment values are coerced the same way as query parameters.
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
    T: vld::schema::VldParse,
{
    type Rejection = VldJsonRejection;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let raw =
            axum::extract::Path::<std::collections::HashMap<String, String>>::from_request_parts(
                parts, state,
            )
            .await
            .map_err(|e| VldJsonRejection::parse(format!("Path parameter error: {}", e)))?;

        let mut map = serde_json::Map::new();
        for (k, v) in raw.0 {
            map.insert(k, coerce_value(&v));
        }
        let value = serde_json::Value::Object(map);

        let parsed = T::vld_parse_value(&value).map_err(VldJsonRejection::validation)?;

        Ok(VldPath(parsed))
    }
}

// ============================= VldForm =======================================

/// Axum extractor that validates **URL-encoded form bodies**
/// (`application/x-www-form-urlencoded`).
///
/// Drop-in replacement for `axum::extract::Form<T>`.
///
/// Values are coerced the same way as query parameters.
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
    T: vld::schema::VldParse,
{
    type Rejection = VldJsonRejection;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let body = <axum::body::Bytes as FromRequest<S>>::from_request(req, state)
            .await
            .map_err(|rejection| {
                let resp = rejection.into_response();
                if resp.status() == StatusCode::PAYLOAD_TOO_LARGE {
                    VldJsonRejection::payload_too_large()
                } else {
                    VldJsonRejection::parse("Failed to read request body")
                }
            })?;

        let body_str = std::str::from_utf8(&body)
            .map_err(|_| VldJsonRejection::parse("Form body is not valid UTF-8"))?;

        let value = query_string_to_json(body_str);

        let parsed = T::vld_parse_value(&value).map_err(VldJsonRejection::validation)?;

        Ok(VldForm(parsed))
    }
}

// ============================= VldHeaders ====================================

/// Axum extractor that validates **HTTP headers**.
///
/// Header names are normalised to snake_case for schema matching:
/// `Content-Type` → `content_type`, `X-Request-Id` → `x_request_id`.
///
/// Values are coerced: `"42"` → number, `"true"` → boolean, etc.
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
    T: vld::schema::VldParse,
{
    type Rejection = VldJsonRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let value = headers_to_json(&parts.headers);

        let parsed = T::vld_parse_value(&value).map_err(VldJsonRejection::validation)?;

        Ok(VldHeaders(parsed))
    }
}

// ============================= VldCookie =====================================

/// Axum extractor that validates **cookie values** from the `Cookie` header.
///
/// Cookie names are used as-is for schema field matching.
/// Values are coerced the same way as query parameters.
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
    T: vld::schema::VldParse,
{
    type Rejection = VldJsonRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let cookie_header = parts
            .headers
            .get(http::header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let value = cookies_to_json(cookie_header);

        let parsed = T::vld_parse_value(&value).map_err(VldJsonRejection::validation)?;

        Ok(VldCookie(parsed))
    }
}

// ========================= Helper functions ==================================

use vld_http_common::{coerce_value, cookies_to_json, query_string_to_json};

/// Build a JSON object from HTTP headers.
///
/// Header names are normalised: `Content-Type` → `content_type`.
fn headers_to_json(headers: &http::HeaderMap) -> serde_json::Value {
    let mut map = serde_json::Map::new();

    for (name, value) in headers.iter() {
        let key = name.as_str().replace('-', "_");
        if let Ok(v) = value.to_str() {
            map.insert(key, coerce_value(v));
        }
    }

    serde_json::Value::Object(map)
}

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{VldCookie, VldForm, VldHeaders, VldJson, VldJsonRejection, VldPath, VldQuery};
    pub use vld::prelude::*;
}
