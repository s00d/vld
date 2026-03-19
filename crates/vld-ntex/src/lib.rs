//! # vld-ntex — ntex integration for the `vld` validation library
//!
//! Provides extractors that validate request data using `vld` schemas:
//!
//! | Extractor | Replaces | Source |
//! |---|---|---|
//! | [`VldJson<T>`] | `ntex::web::types::Json<T>` | JSON request body |
//! | [`VldQuery<T>`] | `ntex::web::types::Query<T>` | URL query parameters |
//! | [`VldPath<T>`] | `ntex::web::types::Path<T>` | URL path parameters |
//! | [`VldForm<T>`] | `ntex::web::types::Form<T>` | URL-encoded form body |
//! | [`VldHeaders<T>`] | manual header extraction | HTTP headers |
//! | [`VldCookie<T>`] | manual cookie parsing | Cookie values |
//!
//! All extractors return **422 Unprocessable Entity** on validation failure.
//!
//! # Quick example
//!
//! ```ignore
//! use ntex::web::{self, App, HttpResponse};
//! use vld::prelude::*;
//! use vld_ntex::{VldJson, VldQuery, VldPath, VldHeaders};
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
//!     pub struct Body {
//!         pub name: String => vld::string().min(2),
//!     }
//! }
//!
//! async fn handler(
//!     path: VldPath<PathParams>,
//!     body: VldJson<Body>,
//! ) -> HttpResponse {
//!     HttpResponse::Ok().body(format!("id={} name={}", path.id, body.name))
//! }
//! ```

use ntex::http::StatusCode;
use ntex::web::error::WebResponseError;
use ntex::web::{ErrorRenderer, FromRequest, HttpRequest, HttpResponse};
use std::fmt;

// ============================= Error / Rejection =============================

/// Error type returned when validation fails.
///
/// Used by all `Vld*` extractors in this crate.
#[derive(Debug)]
pub struct VldNtexError {
    error: vld::error::VldError,
}

impl fmt::Display for VldNtexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Validation failed: {}", self.error)
    }
}

impl<Err: ErrorRenderer> WebResponseError<Err> for VldNtexError {
    fn status_code(&self) -> StatusCode {
        StatusCode::UNPROCESSABLE_ENTITY
    }

    fn error_response(&self, _: &HttpRequest) -> HttpResponse {
        let body = vld_http_common::format_vld_error(&self.error);

        HttpResponse::build(StatusCode::UNPROCESSABLE_ENTITY)
            .header("content-type", "application/json")
            .body(body.to_string())
    }
}

// ============================= VldJson =======================================

/// ntex extractor that validates **JSON request bodies**.
///
/// Drop-in replacement for `ntex::web::types::Json<T>`.
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

impl<T: vld::schema::VldParse, Err: ErrorRenderer> FromRequest<Err> for VldJson<T> {
    type Error = VldNtexError;

    async fn from_request(
        req: &HttpRequest,
        payload: &mut ntex::http::Payload,
    ) -> Result<Self, Self::Error> {
        let json_value =
            <ntex::web::types::Json<serde_json::Value> as FromRequest<Err>>::from_request(
                req, payload,
            )
            .await
            .map_err(|e| VldNtexError {
                error: vld::error::VldError::single(
                    vld::error::IssueCode::ParseError,
                    format!("JSON parse error: {}", e),
                ),
            })?;

        let parsed =
            T::vld_parse_value(&json_value).map_err(|error| VldNtexError { error })?;

        Ok(VldJson(parsed))
    }
}

// ============================= VldQuery ======================================

/// ntex extractor that validates **URL query parameters**.
///
/// Drop-in replacement for `ntex::web::types::Query<T>`.
///
/// Values are coerced: `"42"` -> number, `"true"`/`"false"` -> boolean, empty -> null.
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

impl<T: vld::schema::VldParse, Err: ErrorRenderer> FromRequest<Err> for VldQuery<T> {
    type Error = VldNtexError;

    async fn from_request(
        req: &HttpRequest,
        _payload: &mut ntex::http::Payload,
    ) -> Result<Self, Self::Error> {
        let query_string = req.query_string();
        let value = query_string_to_json(query_string);
        let parsed = T::vld_parse_value(&value).map_err(|error| VldNtexError { error })?;
        Ok(VldQuery(parsed))
    }
}

// ============================= VldPath =======================================

/// ntex extractor that validates **URL path parameters**.
///
/// Drop-in replacement for `ntex::web::types::Path<T>`.
///
/// Path segment values are coerced the same way as query parameters.
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

impl<T: vld::schema::VldParse, Err: ErrorRenderer> FromRequest<Err> for VldPath<T> {
    type Error = VldNtexError;

    async fn from_request(
        req: &HttpRequest,
        _payload: &mut ntex::http::Payload,
    ) -> Result<Self, Self::Error> {
        let mut map = serde_json::Map::new();
        for (name, value) in req.match_info().iter() {
            map.insert(name.to_owned(), coerce_value(value));
        }

        let value = serde_json::Value::Object(map);
        let parsed = T::vld_parse_value(&value).map_err(|error| VldNtexError { error })?;
        Ok(VldPath(parsed))
    }
}

// ============================= VldForm =======================================

/// ntex extractor that validates **URL-encoded form bodies**
/// (`application/x-www-form-urlencoded`).
///
/// Drop-in replacement for `ntex::web::types::Form<T>`.
///
/// Values are coerced the same way as query parameters.
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

impl<T: vld::schema::VldParse, Err: ErrorRenderer> FromRequest<Err> for VldForm<T> {
    type Error = VldNtexError;

    async fn from_request(
        req: &HttpRequest,
        payload: &mut ntex::http::Payload,
    ) -> Result<Self, Self::Error> {
        let bytes =
            <ntex::util::Bytes as FromRequest<Err>>::from_request(req, payload)
                .await
                .map_err(|e| VldNtexError {
                    error: vld::error::VldError::single(
                        vld::error::IssueCode::ParseError,
                        format!("Failed to read form body: {}", e),
                    ),
                })?;

        let body_bytes: &[u8] = &bytes;
        let body_str = std::str::from_utf8(body_bytes).map_err(|_| VldNtexError {
            error: vld::error::VldError::single(
                vld::error::IssueCode::ParseError,
                "Form body is not valid UTF-8",
            ),
        })?;

        let value = query_string_to_json(body_str);
        let parsed = T::vld_parse_value(&value).map_err(|error| VldNtexError { error })?;
        Ok(VldForm(parsed))
    }
}

// ============================= VldHeaders ====================================

/// ntex extractor that validates **HTTP headers**.
///
/// Header names are normalised to snake_case for schema matching:
/// `Content-Type` -> `content_type`, `X-Request-Id` -> `x_request_id`.
///
/// Values are coerced: `"42"` -> number, `"true"` -> boolean, etc.
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

impl<T: vld::schema::VldParse, Err: ErrorRenderer> FromRequest<Err> for VldHeaders<T> {
    type Error = VldNtexError;

    async fn from_request(
        req: &HttpRequest,
        _payload: &mut ntex::http::Payload,
    ) -> Result<Self, Self::Error> {
        let value = headers_to_json(req.headers());
        let parsed = T::vld_parse_value(&value).map_err(|error| VldNtexError { error })?;
        Ok(VldHeaders(parsed))
    }
}

// ============================= VldCookie =====================================

/// ntex extractor that validates **cookie values** from the `Cookie` header.
///
/// Cookie names are used as-is for schema field matching.
/// Values are coerced the same way as query parameters.
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

impl<T: vld::schema::VldParse, Err: ErrorRenderer> FromRequest<Err> for VldCookie<T> {
    type Error = VldNtexError;

    async fn from_request(
        req: &HttpRequest,
        _payload: &mut ntex::http::Payload,
    ) -> Result<Self, Self::Error> {
        let cookie_header = req
            .headers()
            .get(ntex::http::header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let value = cookies_to_json(cookie_header);
        let parsed = T::vld_parse_value(&value).map_err(|error| VldNtexError { error })?;
        Ok(VldCookie(parsed))
    }
}

// ========================= Helper functions ==================================

use vld_http_common::{coerce_value, cookies_to_json, query_string_to_json};

fn headers_to_json(headers: &ntex::http::HeaderMap) -> serde_json::Value {
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
    pub use crate::{VldCookie, VldForm, VldHeaders, VldJson, VldNtexError, VldPath, VldQuery};
    pub use vld::prelude::*;
}
