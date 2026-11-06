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
//! the limit. [`VldJson`] requires a JSON `Content-Type` (`application/json`
//! or `application/*+json`) and [`VldForm`]
//! `application/x-www-form-urlencoded`; anything else is **415 Unsupported
//! Media Type**. See the [error body format](vld_http_common#error-bodies).
//!
//! Guide: <https://s00d.github.io/vld/integrations/web/ntex> (shared rules
//! for all adapters: <https://s00d.github.io/vld/integrations/web/overview>).
//!
//! # Body limits
//!
//! [`VldJson`] and [`VldForm`] read the body themselves up to
//! [`DEFAULT_BODY_LIMIT`] (2 MiB); ntex's `JsonConfig` / `PayloadConfig` do
//! not apply. Register [`VldBodyLimit`] as app state to change it:
//! `App::new().state(VldBodyLimit(n))` (ntex 3) or
//! `App::new().with_config(WebAppConfig::new().set_state(VldBodyLimit(n)))`
//! (ntex 4). Compressed request bodies are decoded only with the `compress`
//! feature (the limit then applies to the decoded size).
//!
//! # ntex versions
//!
//! Features `ntex-4` (default) and `ntex-3` select the ntex major version;
//! enable exactly one. The selected crate is re-exported as [`ntex`].
//!
//! # Example
//!
//! ```no_run
//! use vld_ntex::ntex::web::HttpResponse;
//! use vld_ntex::{VldJson, VldPath};
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
//!
//! // Inside `HttpServer::new(...)`:
//! // App::new().route("/users/{id}", web::post().to(handler))
//! ```
#![warn(missing_docs)]

#[cfg(all(feature = "ntex-3", feature = "ntex-4"))]
compile_error!("Enable exactly one ntex version feature: `ntex-3` or `ntex-4`.");
#[cfg(not(any(feature = "ntex-3", feature = "ntex-4")))]
compile_error!("Enable one ntex version feature: `ntex-3` or `ntex-4`.");

/// The `ntex` crate version selected by the `ntex-3` / `ntex-4` feature.
#[cfg(feature = "ntex-3")]
pub use ntex3 as ntex;
/// The `ntex` crate version selected by the `ntex-3` / `ntex-4` feature.
#[cfg(feature = "ntex-4")]
pub use ntex4 as ntex;

use ntex::http::{Payload, StatusCode, header};
use ntex::web::{HttpRequest, HttpResponse, HttpResponseBuilder};
use std::fmt;
use vld::schema::VldParse;
pub use vld_http_common::VldBodyLimit;
use vld_http_common::{
    DEFAULT_BODY_LIMIT, VldHttpError, cookie_headers_to_json, headers_to_json, parse_coerced,
    parse_form_body, parse_json_body, path_params_to_json, query_string_to_json,
    require_form_content_type, require_json_content_type,
};

// ============================= Error / Rejection =============================

/// Error type returned when validation or body reading fails.
///
/// Used by all `Vld*` extractors in this crate; wraps [`VldHttpError`].
#[derive(Debug)]
pub struct VldNtexError(VldHttpError);

impl VldNtexError {
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

impl std::error::Error for VldNtexError {}

impl VldNtexError {
    fn render(&self) -> HttpResponse {
        HttpResponseBuilder::new(self.status())
            .header("content-type", "application/json")
            .body(self.0.body().to_string())
    }
}

/// Read the body up to the [`VldBodyLimit`] app state, or
/// [`DEFAULT_BODY_LIMIT`].
async fn read_body(req: &HttpRequest, payload: &mut Payload) -> Result<Vec<u8>, VldHttpError> {
    let limit = req
        .app_state::<VldBodyLimit>()
        .map_or(DEFAULT_BODY_LIMIT, |l| l.0);
    // With `compress`, decode `Content-Encoding` like ntex's own body
    // extractors; the limit applies to the decoded size.
    #[cfg(feature = "compress")]
    let mut stream = ntex::http::encoding::Decoder::from_headers(payload.take(), req.headers());
    #[cfg(not(feature = "compress"))]
    let mut stream = payload.take();
    let mut body = Vec::new();
    while let Some(chunk) = ntex::util::stream_recv(&mut stream).await {
        let chunk =
            chunk.map_err(|e| VldHttpError::parse(format!("Failed to read request body: {e}")))?;
        if body.len() + chunk.len() > limit {
            return Err(VldHttpError::payload_too_large());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn content_type(req: &HttpRequest) -> Option<&str> {
    req.headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
}

/// Validate a value coerced from text (query, path, form, headers, cookies).
fn validate<T: VldParse>(value: &serde_json::Value) -> Result<T, VldNtexError> {
    parse_coerced(value).map_err(VldNtexError::from)
}

// ============================= Version glue ==================================
//
// The only ntex-version-specific code: error rendering and the `FromRequest`
// impls (generated by `from_request!`, forwarding to each extractor's
// inherent `extract`).

#[cfg(feature = "ntex-3")]
mod compat {
    use super::*;
    use ntex::web::error::WebResponseError;
    use ntex::web::{ErrorRenderer, FromRequest};

    impl<Err: ErrorRenderer> WebResponseError<Err> for VldNtexError {
        fn status_code(&self) -> StatusCode {
            self.status()
        }

        fn error_response(&self, _: &HttpRequest) -> HttpResponse {
            self.render()
        }
    }
    macro_rules! from_request {
        ($($t:ident),+) => {$(
            impl<T: VldParse, Err: ErrorRenderer> FromRequest<Err> for $t<T> {
                type Error = VldNtexError;

                async fn from_request(
                    req: &HttpRequest,
                    payload: &mut Payload,
                ) -> Result<Self, Self::Error> {
                    Self::extract(req, payload).await
                }
            }
        )+};
    }
    from_request!(VldJson, VldQuery, VldPath, VldForm, VldHeaders, VldCookie);
}

#[cfg(feature = "ntex-4")]
mod compat {
    use super::*;
    use ntex::web::FromRequest;
    use ntex::web::error::WebResponseError;

    impl<St, Err> WebResponseError<St, Err> for VldNtexError {
        fn error_response(&self, _: &St) -> HttpResponse {
            self.render()
        }
    }
    macro_rules! from_request {
        ($($t:ident),+) => {$(
            impl<T: VldParse, St> FromRequest<St> for $t<T> {
                type Error = VldNtexError;

                async fn from_request(
                    _: &St,
                    req: &HttpRequest,
                    payload: &mut Payload,
                ) -> Result<Self, Self::Error> {
                    Self::extract(req, payload).await
                }
            }
        )+};
    }
    from_request!(VldJson, VldQuery, VldPath, VldForm, VldHeaders, VldCookie);
}

// ============================= VldJson =======================================

/// ntex extractor that validates **JSON request bodies**.
///
/// Drop-in replacement for `ntex::web::types::Json<T>`.
pub struct VldJson<T>(pub T);

impl<T: VldParse> VldJson<T> {
    async fn extract(req: &HttpRequest, payload: &mut Payload) -> Result<Self, VldNtexError> {
        require_json_content_type(content_type(req))?;
        let body = read_body(req, payload).await?;
        T::vld_parse_value(&parse_json_body(&body)?)
            .map(VldJson)
            .map_err(VldNtexError::from)
    }
}

// ============================= VldQuery ======================================

/// ntex extractor that validates **URL query parameters**.
///
/// Drop-in replacement for `ntex::web::types::Query<T>`.
///
/// See [`vld_http_common::parse_query_string`] for coercion and array rules.
pub struct VldQuery<T>(pub T);

impl<T: VldParse> VldQuery<T> {
    async fn extract(req: &HttpRequest, _payload: &mut Payload) -> Result<Self, VldNtexError> {
        validate(&query_string_to_json(req.query_string())).map(VldQuery)
    }
}

// ============================= VldPath =======================================

/// ntex extractor that validates **URL path parameters**.
///
/// Drop-in replacement for `ntex::web::types::Path<T>`.
///
/// Path segment values come percent-decoded from ntex's router (`%2F`,
/// `%25`, `%2B` included) and are coerced the same way as query parameters.
pub struct VldPath<T>(pub T);

impl<T: VldParse> VldPath<T> {
    async fn extract(req: &HttpRequest, _payload: &mut Payload) -> Result<Self, VldNtexError> {
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

impl<T: VldParse> VldForm<T> {
    async fn extract(req: &HttpRequest, payload: &mut Payload) -> Result<Self, VldNtexError> {
        require_form_content_type(content_type(req))?;
        let body = read_body(req, payload).await?;
        validate(&parse_form_body(&body)?).map(VldForm)
    }
}

// ============================= VldHeaders ====================================

/// ntex extractor that validates **HTTP headers**.
///
/// See [`vld_http_common::headers_to_json`]: names become snake_case
/// (`X-Request-Id` → `x_request_id`), names containing `_` are ignored,
/// repeated headers are joined with `", "`.
pub struct VldHeaders<T>(pub T);

impl<T: VldParse> VldHeaders<T> {
    async fn extract(req: &HttpRequest, _payload: &mut Payload) -> Result<Self, VldNtexError> {
        let value = headers_to_json(
            req.headers()
                .iter()
                .map(|(n, v)| (n.as_str(), v.as_bytes())),
        );
        validate(&value).map(VldHeaders)
    }
}

// ============================= VldCookie =====================================

/// ntex extractor that validates **cookie values** from the `Cookie`
/// headers (all of them; the first occurrence of a name wins).
///
/// Cookie names are used as-is for schema field matching.
/// Values are unquoted, percent-decoded and coerced.
pub struct VldCookie<T>(pub T);

impl<T: VldParse> VldCookie<T> {
    async fn extract(req: &HttpRequest, _payload: &mut Payload) -> Result<Self, VldNtexError> {
        let value =
            cookie_headers_to_json(req.headers().get_all(header::COOKIE).map(|v| v.as_bytes()));
        validate(&value).map(VldCookie)
    }
}

vld_http_common::impl_deref!(VldJson, VldQuery, VldPath, VldForm, VldHeaders, VldCookie);

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{
        VldBodyLimit, VldCookie, VldForm, VldHeaders, VldJson, VldNtexError, VldPath, VldQuery,
    };
    pub use vld::prelude::*;
}
