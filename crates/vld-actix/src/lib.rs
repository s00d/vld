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
//! the limit. [`VldJson`] requires a JSON `Content-Type` (`application/json`
//! or `application/*+json`) and [`VldForm`]
//! `application/x-www-form-urlencoded`; anything else is **415 Unsupported
//! Media Type**. See the [error body format](vld_http_common#error-bodies).
//!
//! Guide: <https://s00d.github.io/vld/integrations/web/actix> (shared rules
//! for all adapters: <https://s00d.github.io/vld/integrations/web/overview>).
//!
//! # Body limits
//!
//! [`VldJson`] and [`VldForm`] read the body themselves (compressed bodies
//! are decoded; the limit applies to the decoded size) up to
//! [`DEFAULT_BODY_LIMIT`] (2 MiB). Register [`VldBodyLimit`] with
//! `app_data` to change it — actix's `JsonConfig` / `PayloadConfig` do not
//! apply:
//!
//! ```no_run
//! use actix_web::App;
//!
//! let app = App::new().app_data(vld_actix::VldBodyLimit(8 * 1024 * 1024));
//! ```
//!
//! # Example
//!
//! ```no_run
//! use actix_web::{web, App, HttpResponse};
//! use vld_actix::{VldHeaders, VldJson, VldPath};
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
//!
//! let app = App::new().route("/users/{id}", web::post().to(handler));
//! ```

use actix_web::body::{BodyStream, to_bytes_limited};
use actix_web::dev::{Decompress, Payload};
use actix_web::http::{StatusCode, header};
use actix_web::{FromRequest, HttpRequest, HttpResponse, ResponseError};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
pub use vld_http_common::VldBodyLimit;
use vld_http_common::{
    DEFAULT_BODY_LIMIT, VldHttpError, cookie_headers_to_json, headers_to_json, parse_coerced,
    parse_form_body, parse_json_body, path_params_to_json, percent_decode, query_string_to_json,
    require_form_content_type, require_json_content_type,
};

// ============================= Error / Rejection =============================

/// Error type returned when validation or body reading fails.
///
/// Used by all `Vld*` extractors in this crate; wraps [`VldHttpError`].
#[derive(Debug)]
pub struct VldActixError(VldHttpError);

/// Former name of [`VldActixError`] (it is used by every extractor, not
/// only [`VldJson`]).
#[deprecated(since = "0.5.0", note = "renamed to `VldActixError`")]
pub type VldJsonError = VldActixError;

impl VldActixError {
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

impl From<VldHttpError> for VldActixError {
    fn from(e: VldHttpError) -> Self {
        Self(e)
    }
}

impl From<vld::error::VldError> for VldActixError {
    fn from(e: vld::error::VldError) -> Self {
        Self(e.into())
    }
}

impl fmt::Display for VldActixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl ResponseError for VldActixError {
    fn status_code(&self) -> StatusCode {
        self.status()
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .content_type("application/json")
            .body(self.0.body().to_string())
    }
}

/// Read the (decompressed) body up to the [`VldBodyLimit`] app data, or
/// [`DEFAULT_BODY_LIMIT`].
fn read_body(
    req: &HttpRequest,
    payload: &mut Payload,
) -> impl Future<Output = Result<actix_web::web::Bytes, VldHttpError>> + 'static + use<> {
    let limit = req
        .app_data::<VldBodyLimit>()
        .map_or(DEFAULT_BODY_LIMIT, |l| l.0);
    let stream = Decompress::from_headers(payload.take(), req.headers());
    async move {
        match to_bytes_limited(BodyStream::new(stream), limit).await {
            Ok(Ok(bytes)) => Ok(bytes),
            Ok(Err(e)) => Err(VldHttpError::parse(format!(
                "Failed to read request body: {e}"
            ))),
            Err(_) => Err(VldHttpError::payload_too_large()),
        }
    }
}

fn content_type(req: &HttpRequest) -> Option<&str> {
    req.headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
}

/// Validate a JSON value against `T`.
fn validate<T: vld::schema::VldParse>(value: &serde_json::Value) -> Result<T, VldActixError> {
    T::vld_parse_value(value).map_err(VldActixError::from)
}

/// Validate a value coerced from text (query, path, form, headers, cookies).
fn validate_coerced<T: vld::schema::VldParse>(
    value: &serde_json::Value,
) -> Result<T, VldActixError> {
    parse_coerced(value).map_err(VldActixError::from)
}

/// Extractor future type.
type ExtractFuture<T> = Pin<Box<dyn Future<Output = Result<T, VldActixError>>>>;

// ============================= VldJson =======================================

/// Actix-web extractor that validates **JSON request bodies**.
///
/// Drop-in replacement for `actix_web::web::Json<T>`.
pub struct VldJson<T>(pub T);

impl<T: vld::schema::VldParse> FromRequest for VldJson<T> {
    type Error = VldActixError;
    type Future = ExtractFuture<Self>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        if let Err(e) = require_json_content_type(content_type(req)) {
            return Box::pin(async move { Err(e.into()) });
        }
        let body = read_body(req, payload);
        Box::pin(async move { validate(&parse_json_body(&body.await?)?).map(VldJson) })
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
    type Error = VldActixError;
    type Future = ExtractFuture<Self>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let value = query_string_to_json(req.query_string());
        Box::pin(async move { validate_coerced(&value).map(VldQuery) })
    }
}

// ============================= VldPath =======================================

/// Actix-web extractor that validates **URL path parameters**.
///
/// Drop-in replacement for `actix_web::web::Path<T>`. Works with custom
/// segment patterns such as `{id:\d+}`.
///
/// Path segment values are percent-decoded (actix keeps `%2F`, `%25` and
/// `%2B` encoded in `match_info`) and coerced the same way as query
/// parameters.
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
    type Error = VldActixError;
    type Future = ExtractFuture<Self>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let params: Vec<_> = req
            .match_info()
            .iter()
            .map(|(k, v)| (k, percent_decode(v)))
            .collect();
        let value = path_params_to_json(params.iter().map(|(k, v)| (*k, v.as_str())));
        Box::pin(async move { validate_coerced(&value).map(VldPath) })
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
    type Error = VldActixError;
    type Future = ExtractFuture<Self>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        if let Err(e) = require_form_content_type(content_type(req)) {
            return Box::pin(async move { Err(e.into()) });
        }
        let body = read_body(req, payload);
        Box::pin(async move { validate_coerced(&parse_form_body(&body.await?)?).map(VldForm) })
    }
}

// ============================= VldHeaders ====================================

/// Actix-web extractor that validates **HTTP headers**.
///
/// See [`vld_http_common::headers_to_json`]: names become snake_case
/// (`X-Request-Id` → `x_request_id`), names containing `_` are ignored,
/// repeated headers are joined with `", "`.
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
    type Error = VldActixError;
    type Future = ExtractFuture<Self>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let value = headers_to_json(
            req.headers()
                .iter()
                .map(|(n, v)| (n.as_str(), v.as_bytes())),
        );
        Box::pin(async move { validate_coerced(&value).map(VldHeaders) })
    }
}

// ============================= VldCookie =====================================

/// Actix-web extractor that validates **cookie values** from the `Cookie`
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
/// async fn dashboard(cookies: VldCookie<SessionCookies>) -> HttpResponse {
///     HttpResponse::Ok().body(format!("session={}", cookies.session_id))
/// }
/// ```
pub struct VldCookie<T>(pub T);

impl<T: vld::schema::VldParse> FromRequest for VldCookie<T> {
    type Error = VldActixError;
    type Future = ExtractFuture<Self>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let value =
            cookie_headers_to_json(req.headers().get_all(header::COOKIE).map(|v| v.as_bytes()));
        Box::pin(async move { validate_coerced(&value).map(VldCookie) })
    }
}

vld_http_common::impl_deref!(VldJson, VldQuery, VldPath, VldForm, VldHeaders, VldCookie);

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{
        VldActixError, VldBodyLimit, VldCookie, VldForm, VldHeaders, VldJson, VldPath, VldQuery,
    };
    pub use vld::prelude::*;
}
