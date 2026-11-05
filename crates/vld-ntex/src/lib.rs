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
//! All extractors return **422 Unprocessable Entity** on validation failure
//! or malformed input, and **413 Payload Too Large** when the body exceeds
//! the limit.
//!
//! # Body limits
//!
//! Body reading is delegated to ntex, so its configuration applies:
//! `JsonConfig` for [`VldJson`] (ntex default 32 KiB) and `PayloadConfig`
//! for [`VldForm`] (ntex default 256 KiB). Register e.g.
//! `App::new().state(JsonConfig::default().limit(vld_http_common::DEFAULT_BODY_LIMIT))`
//! to use the same 2 MiB default as the other `vld` adapters.
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

use ntex::http::error::PayloadError;
use ntex::http::StatusCode;
use ntex::web::error::{JsonPayloadError, WebResponseError};
use ntex::web::{ErrorRenderer, FromRequest, HttpRequest, HttpResponse};
use std::fmt;
use vld::schema::VldParse;
use vld_http_common::{
    cookies_to_json, headers_to_json, parse_form_body, path_params_to_json, query_string_to_json,
    VldHttpError,
};

// ============================= Error / Rejection =============================

/// Error type returned when validation or body reading fails.
///
/// Used by all `Vld*` extractors in this crate.
#[derive(Debug)]
pub struct VldNtexError(VldHttpError);

impl VldNtexError {
    /// The underlying validation error.
    pub fn error(&self) -> &vld::error::VldError {
        self.0.error()
    }
}

impl From<VldHttpError> for VldNtexError {
    fn from(e: VldHttpError) -> Self {
        Self(e)
    }
}

impl From<vld::error::VldError> for VldNtexError {
    fn from(e: vld::error::VldError) -> Self {
        Self(e.into())
    }
}

impl fmt::Display for VldNtexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<Err: ErrorRenderer> WebResponseError<Err> for VldNtexError {
    fn status_code(&self) -> StatusCode {
        StatusCode::from_u16(self.0.status()).unwrap_or(StatusCode::UNPROCESSABLE_ENTITY)
    }

    fn error_response(&self, _: &HttpRequest) -> HttpResponse {
        HttpResponse::build(<Self as WebResponseError<Err>>::status_code(self))
            .header("content-type", "application/json")
            .body(self.0.body().to_string())
    }
}

/// `PayloadError::Overflow` → 413, anything else → 422.
fn payload_error(e: PayloadError, what: &str) -> VldNtexError {
    match e {
        PayloadError::Overflow => VldHttpError::payload_too_large().into(),
        other => VldHttpError::parse(format!("{what}: {other}")).into(),
    }
}

fn validate<T: VldParse>(value: &serde_json::Value) -> Result<T, VldNtexError> {
    T::vld_parse_value(value).map_err(VldNtexError::from)
}

// ============================= VldJson =======================================

/// ntex extractor that validates **JSON request bodies**.
///
/// Drop-in replacement for `ntex::web::types::Json<T>`.
pub struct VldJson<T>(pub T);

impl<T: VldParse, Err: ErrorRenderer> FromRequest<Err> for VldJson<T> {
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
            .map_err(|e| match e {
                JsonPayloadError::Overflow => VldHttpError::payload_too_large().into(),
                JsonPayloadError::Payload(p) => payload_error(p, "JSON parse error"),
                other => {
                    VldNtexError::from(VldHttpError::parse(format!("JSON parse error: {other}")))
                }
            })?
            .into_inner();

        validate(&json_value).map(VldJson)
    }
}

// ============================= VldQuery ======================================

/// ntex extractor that validates **URL query parameters**.
///
/// Drop-in replacement for `ntex::web::types::Query<T>`.
///
/// See [`vld_http_common::parse_query_string`] for coercion and array rules.
pub struct VldQuery<T>(pub T);

impl<T: VldParse, Err: ErrorRenderer> FromRequest<Err> for VldQuery<T> {
    type Error = VldNtexError;

    async fn from_request(
        req: &HttpRequest,
        _payload: &mut ntex::http::Payload,
    ) -> Result<Self, Self::Error> {
        validate(&query_string_to_json(req.query_string())).map(VldQuery)
    }
}

// ============================= VldPath =======================================

/// ntex extractor that validates **URL path parameters**.
///
/// Drop-in replacement for `ntex::web::types::Path<T>`.
///
/// Path segment values are coerced the same way as query parameters.
pub struct VldPath<T>(pub T);

impl<T: VldParse, Err: ErrorRenderer> FromRequest<Err> for VldPath<T> {
    type Error = VldNtexError;

    async fn from_request(
        req: &HttpRequest,
        _payload: &mut ntex::http::Payload,
    ) -> Result<Self, Self::Error> {
        validate(&path_params_to_json(req.match_info().iter())).map(VldPath)
    }
}

// ============================= VldForm =======================================

/// ntex extractor that validates **URL-encoded form bodies**
/// (`application/x-www-form-urlencoded`).
///
/// Drop-in replacement for `ntex::web::types::Form<T>`.
///
/// Values are parsed the same way as query parameters.
pub struct VldForm<T>(pub T);

impl<T: VldParse, Err: ErrorRenderer> FromRequest<Err> for VldForm<T> {
    type Error = VldNtexError;

    async fn from_request(
        req: &HttpRequest,
        payload: &mut ntex::http::Payload,
    ) -> Result<Self, Self::Error> {
        let bytes = <ntex::util::Bytes as FromRequest<Err>>::from_request(req, payload)
            .await
            .map_err(|e| match e {
                ntex::web::error::PayloadError::Payload(p) => {
                    payload_error(p, "Failed to read form body")
                }
                other => VldHttpError::parse(format!("Failed to read form body: {other}")).into(),
            })?;

        validate(&parse_form_body(&bytes)?).map(VldForm)
    }
}

// ============================= VldHeaders ====================================

/// ntex extractor that validates **HTTP headers**.
///
/// See [`vld_http_common::headers_to_json`]: names become snake_case
/// (`X-Request-Id` → `x_request_id`), repeated headers are joined with `", "`.
pub struct VldHeaders<T>(pub T);

impl<T: VldParse, Err: ErrorRenderer> FromRequest<Err> for VldHeaders<T> {
    type Error = VldNtexError;

    async fn from_request(
        req: &HttpRequest,
        _payload: &mut ntex::http::Payload,
    ) -> Result<Self, Self::Error> {
        let value = headers_to_json(
            req.headers()
                .iter()
                .map(|(n, v)| (n.as_str(), v.as_bytes())),
        );
        validate(&value).map(VldHeaders)
    }
}

// ============================= VldCookie =====================================

/// ntex extractor that validates **cookie values** from the `Cookie` header.
///
/// Cookie names are used as-is for schema field matching.
/// Values are unquoted, percent-decoded and coerced.
pub struct VldCookie<T>(pub T);

impl<T: VldParse, Err: ErrorRenderer> FromRequest<Err> for VldCookie<T> {
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
        validate(&cookies_to_json(cookie_header)).map(VldCookie)
    }
}

vld_http_common::impl_deref!(VldJson, VldQuery, VldPath, VldForm, VldHeaders, VldCookie);

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{VldCookie, VldForm, VldHeaders, VldJson, VldNtexError, VldPath, VldQuery};
    pub use vld::prelude::*;
}
