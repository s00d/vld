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
//! All extractors return **422 Unprocessable Entity** on validation failure
//! or malformed input, and **413 Payload Too Large** when the body exceeds
//! the limit.
//!
//! # Body limits
//!
//! Body reading is delegated to actix, so its configuration applies:
//! [`JsonConfig`](actix_web::web::JsonConfig) for [`VldJson`] (default 2 MiB)
//! and [`PayloadConfig`](actix_web::web::PayloadConfig) for [`VldForm`]
//! (default 256 KiB).
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
use actix_web::http::StatusCode;
use actix_web::{FromRequest, HttpRequest, HttpResponse, ResponseError};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use vld_http_common::{
    cookies_to_json, headers_to_json, parse_form_body, path_params_to_json, query_string_to_json,
    VldHttpError,
};

// ============================= Error / Rejection =============================

/// Error type returned when validation or body reading fails.
///
/// Used by all `Vld*` extractors in this crate.
#[derive(Debug)]
pub struct VldJsonError(VldHttpError);

impl VldJsonError {
    /// The underlying validation error.
    pub fn error(&self) -> &vld::error::VldError {
        self.0.error()
    }
}

impl From<VldHttpError> for VldJsonError {
    fn from(e: VldHttpError) -> Self {
        Self(e)
    }
}

impl From<vld::error::VldError> for VldJsonError {
    fn from(e: vld::error::VldError) -> Self {
        Self(e.into())
    }
}

impl fmt::Display for VldJsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl ResponseError for VldJsonError {
    fn status_code(&self) -> StatusCode {
        StatusCode::from_u16(self.0.status()).unwrap_or(StatusCode::UNPROCESSABLE_ENTITY)
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .content_type("application/json")
            .body(self.0.body().to_string())
    }
}

/// Map an actix body-reading error: 413 stays 413, everything else is 422.
fn body_error(e: actix_web::Error, what: &str) -> VldJsonError {
    if e.as_response_error().status_code() == StatusCode::PAYLOAD_TOO_LARGE {
        VldHttpError::payload_too_large().into()
    } else {
        VldHttpError::parse(format!("{what}: {e}")).into()
    }
}

/// Validate a JSON value against `T`.
fn validate<T: vld::schema::VldParse>(value: &serde_json::Value) -> Result<T, VldJsonError> {
    T::vld_parse_value(value).map_err(VldJsonError::from)
}

/// Extractor future type.
type ExtractFuture<T> = Pin<Box<dyn Future<Output = Result<T, VldJsonError>>>>;

// ============================= VldJson =======================================

/// Actix-web extractor that validates **JSON request bodies**.
///
/// Drop-in replacement for `actix_web::web::Json<T>`.
pub struct VldJson<T>(pub T);

impl<T: vld::schema::VldParse> FromRequest for VldJson<T> {
    type Error = VldJsonError;
    type Future = ExtractFuture<Self>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let json_fut = actix_web::web::Json::<serde_json::Value>::from_request(req, payload);

        Box::pin(async move {
            let json_value = json_fut
                .await
                .map_err(|e| body_error(e, "JSON parse error"))?;
            validate(&json_value).map(VldJson)
        })
    }
}

// ============================= VldQuery ======================================

/// Actix-web extractor that validates **URL query parameters**.
///
/// Drop-in replacement for `actix_web::web::Query<T>`.
///
/// See [`vld_http_common::parse_query_string`] for coercion and array rules.
pub struct VldQuery<T>(pub T);

impl<T: vld::schema::VldParse> FromRequest for VldQuery<T> {
    type Error = VldJsonError;
    type Future = ExtractFuture<Self>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let value = query_string_to_json(req.query_string());
        Box::pin(async move { validate(&value).map(VldQuery) })
    }
}

// ============================= VldPath =======================================

/// Actix-web extractor that validates **URL path parameters**.
///
/// Drop-in replacement for `actix_web::web::Path<T>`. Works with custom
/// segment patterns such as `{id:\d+}`.
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

impl<T: vld::schema::VldParse> FromRequest for VldPath<T> {
    type Error = VldJsonError;
    type Future = ExtractFuture<Self>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let value = path_params_to_json(req.match_info().iter());
        Box::pin(async move { validate(&value).map(VldPath) })
    }
}

// ============================= VldForm =======================================

/// Actix-web extractor that validates **URL-encoded form bodies**
/// (`application/x-www-form-urlencoded`).
///
/// Drop-in replacement for `actix_web::web::Form<T>`.
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
/// async fn login(form: VldForm<LoginForm>) -> HttpResponse {
///     HttpResponse::Ok().body(format!("Welcome, {}!", form.username))
/// }
/// ```
pub struct VldForm<T>(pub T);

impl<T: vld::schema::VldParse> FromRequest for VldForm<T> {
    type Error = VldJsonError;
    type Future = ExtractFuture<Self>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let bytes_fut = actix_web::web::Bytes::from_request(req, payload);

        Box::pin(async move {
            let body = bytes_fut
                .await
                .map_err(|e| body_error(e, "Failed to read form body"))?;
            validate(&parse_form_body(&body)?).map(VldForm)
        })
    }
}

// ============================= VldHeaders ====================================

/// Actix-web extractor that validates **HTTP headers**.
///
/// See [`vld_http_common::headers_to_json`]: names become snake_case
/// (`X-Request-Id` → `x_request_id`), repeated headers are joined with `", "`.
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

impl<T: vld::schema::VldParse> FromRequest for VldHeaders<T> {
    type Error = VldJsonError;
    type Future = ExtractFuture<Self>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let value = headers_to_json(
            req.headers()
                .iter()
                .map(|(n, v)| (n.as_str(), v.as_bytes())),
        );
        Box::pin(async move { validate(&value).map(VldHeaders) })
    }
}

// ============================= VldCookie =====================================

/// Actix-web extractor that validates **cookie values** from the `Cookie` header.
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
/// async fn dashboard(cookies: VldCookie<SessionCookies>) -> HttpResponse {
///     HttpResponse::Ok().body(format!("session={}", cookies.session_id))
/// }
/// ```
pub struct VldCookie<T>(pub T);

impl<T: vld::schema::VldParse> FromRequest for VldCookie<T> {
    type Error = VldJsonError;
    type Future = ExtractFuture<Self>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let cookie_header = req
            .headers()
            .get(actix_web::http::header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let value = cookies_to_json(cookie_header);
        Box::pin(async move { validate(&value).map(VldCookie) })
    }
}

vld_http_common::impl_deref!(VldJson, VldQuery, VldPath, VldForm, VldHeaders, VldCookie);

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{VldCookie, VldForm, VldHeaders, VldJson, VldJsonError, VldPath, VldQuery};
    pub use vld::prelude::*;
}
