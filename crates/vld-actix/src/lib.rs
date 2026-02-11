//! # vld-actix — Actix-web integration for the `vld` validation library
//!
//! Provides extractors that validate request data using `vld` schemas:
//!
//! | Extractor | Replaces | Source |
//! |---|---|---|
//! | [`VldJson<T>`] | `actix_web::web::Json<T>` | JSON request body |
//! | [`VldQuery<T>`] | `actix_web::web::Query<T>` | URL query parameters |
//! | [`VldPath<T>`] | `actix_web::web::Path<T>` | URL path parameters |
//! | [`VldForm<T>`] | `actix_web::web::Form<T>` | URL-encoded form body |
//! | [`VldHeaders<T>`] | manual header extraction | HTTP headers |
//! | [`VldCookie<T>`] | manual cookie parsing | Cookie values |
//!
//! All extractors return **422 Unprocessable Entity** on validation failure.
//!
//! # Quick example
//!
//! ```ignore
//! use actix_web::{web, App, HttpResponse};
//! use vld::prelude::*;
//! use vld_actix::{VldPath, VldQuery, VldJson, VldHeaders};
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
//!     path: VldPath<PathParams>,
//!     headers: VldHeaders<Auth>,
//!     body: VldJson<Body>,
//! ) -> HttpResponse {
//!     HttpResponse::Ok().body(format!(
//!         "id={} auth={} name={}",
//!         path.id, headers.authorization, body.name,
//!     ))
//! }
//! ```

use actix_web::dev::Payload;
use actix_web::{FromRequest, HttpRequest, HttpResponse, ResponseError};
use std::fmt;
use std::future::Future;
use std::pin::Pin;

// ============================= Error / Rejection =============================

/// Error type returned when validation fails.
///
/// Used by all `Vld*` extractors in this crate.
#[derive(Debug)]
pub struct VldJsonError {
    error: vld::error::VldError,
}

impl fmt::Display for VldJsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Validation failed: {}", self.error)
    }
}

impl ResponseError for VldJsonError {
    fn error_response(&self) -> HttpResponse {
        let errors: Vec<serde_json::Value> = self
            .error
            .issues
            .iter()
            .map(|issue| {
                let path: String = issue.path.iter().map(|p| p.to_string()).collect();
                serde_json::json!({
                    "path": path,
                    "message": issue.message,
                    "code": issue.code.key(),
                })
            })
            .collect();

        let body = serde_json::json!({
            "error": "Validation failed",
            "issues": errors,
        });

        HttpResponse::UnprocessableEntity()
            .content_type("application/json")
            .body(body.to_string())
    }
}

// ============================= VldJson =======================================

/// Actix-web extractor that validates **JSON request bodies**.
///
/// Drop-in replacement for `actix_web::web::Json<T>`.
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

impl<T: vld::schema::VldParse> FromRequest for VldJson<T> {
    type Error = VldJsonError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let json_fut = actix_web::web::Json::<serde_json::Value>::from_request(req, payload);

        Box::pin(async move {
            let json_value = json_fut.await.map_err(|e| VldJsonError {
                error: vld::error::VldError::single(
                    vld::error::IssueCode::ParseError,
                    format!("JSON parse error: {}", e),
                ),
            })?;

            let parsed = T::vld_parse_value(&json_value).map_err(|error| VldJsonError { error })?;

            Ok(VldJson(parsed))
        })
    }
}

// ============================= VldQuery ======================================

/// Actix-web extractor that validates **URL query parameters**.
///
/// Drop-in replacement for `actix_web::web::Query<T>`.
///
/// Values are coerced: `"42"` → number, `"true"`/`"false"` → boolean, empty → null.
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

impl<T: vld::schema::VldParse> FromRequest for VldQuery<T> {
    type Error = VldJsonError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let query_string = req.query_string().to_owned();

        Box::pin(async move {
            let value = query_string_to_json(&query_string);

            let parsed = T::vld_parse_value(&value).map_err(|error| VldJsonError { error })?;

            Ok(VldQuery(parsed))
        })
    }
}

// ============================= VldPath =======================================

/// Actix-web extractor that validates **URL path parameters**.
///
/// Drop-in replacement for `actix_web::web::Path<T>`.
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
/// async fn get_post(path: VldPath<PostPath>) -> HttpResponse {
///     HttpResponse::Ok().body(format!("user {} post {}", path.id, path.post_id))
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

impl<T: vld::schema::VldParse> FromRequest for VldPath<T> {
    type Error = VldJsonError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let mut map = serde_json::Map::new();

        // Extract param names from the matched route pattern, e.g. "/users/{id}"
        if let Some(pattern) = req.match_pattern() {
            for name in extract_path_param_names(&pattern) {
                if let Some(value) = req.match_info().get(&name) {
                    map.insert(name, coerce_value(value));
                }
            }
        }

        let value = serde_json::Value::Object(map);

        Box::pin(async move {
            let parsed = T::vld_parse_value(&value).map_err(|error| VldJsonError { error })?;

            Ok(VldPath(parsed))
        })
    }
}

// ============================= VldForm =======================================

/// Actix-web extractor that validates **URL-encoded form bodies**
/// (`application/x-www-form-urlencoded`).
///
/// Drop-in replacement for `actix_web::web::Form<T>`.
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
/// async fn login(form: VldForm<LoginForm>) -> HttpResponse {
///     HttpResponse::Ok().body(format!("Welcome, {}!", form.username))
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

impl<T: vld::schema::VldParse> FromRequest for VldForm<T> {
    type Error = VldJsonError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let bytes_fut = actix_web::web::Bytes::from_request(req, payload);

        Box::pin(async move {
            let body = bytes_fut.await.map_err(|e| VldJsonError {
                error: vld::error::VldError::single(
                    vld::error::IssueCode::ParseError,
                    format!("Failed to read form body: {}", e),
                ),
            })?;

            let body_str = std::str::from_utf8(&body).map_err(|_| VldJsonError {
                error: vld::error::VldError::single(
                    vld::error::IssueCode::ParseError,
                    "Form body is not valid UTF-8",
                ),
            })?;

            let value = query_string_to_json(body_str);

            let parsed = T::vld_parse_value(&value).map_err(|error| VldJsonError { error })?;

            Ok(VldForm(parsed))
        })
    }
}

// ============================= VldHeaders ====================================

/// Actix-web extractor that validates **HTTP headers**.
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
/// async fn handler(headers: VldHeaders<RequiredHeaders>) -> HttpResponse {
///     HttpResponse::Ok().body(format!("auth={}", headers.authorization))
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

impl<T: vld::schema::VldParse> FromRequest for VldHeaders<T> {
    type Error = VldJsonError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let value = headers_to_json(req.headers());

        Box::pin(async move {
            let parsed = T::vld_parse_value(&value).map_err(|error| VldJsonError { error })?;

            Ok(VldHeaders(parsed))
        })
    }
}

// ============================= VldCookie =====================================

/// Actix-web extractor that validates **cookie values** from the `Cookie` header.
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
/// async fn dashboard(cookies: VldCookie<SessionCookies>) -> HttpResponse {
///     HttpResponse::Ok().body(format!("session={}", cookies.session_id))
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

impl<T: vld::schema::VldParse> FromRequest for VldCookie<T> {
    type Error = VldJsonError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let cookie_header = req
            .headers()
            .get(actix_web::http::header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();

        Box::pin(async move {
            let value = cookies_to_json(&cookie_header);

            let parsed = T::vld_parse_value(&value).map_err(|error| VldJsonError { error })?;

            Ok(VldCookie(parsed))
        })
    }
}

// ========================= Helper functions ==================================

use vld_http_common::{
    coerce_value, cookies_to_json, extract_path_param_names, query_string_to_json,
};

/// Build a JSON object from HTTP headers.
///
/// Header names are normalised: `content-type` → `content_type`.
fn headers_to_json(headers: &actix_web::http::header::HeaderMap) -> serde_json::Value {
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
    pub use crate::{VldCookie, VldForm, VldHeaders, VldJson, VldJsonError, VldPath, VldQuery};
    pub use vld::prelude::*;
}
