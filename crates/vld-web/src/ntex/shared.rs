//! The ntex adapter, written once against `super::ntex` and compiled by
//! `ntex3.rs` / `ntex4.rs`; what differs between the majors is in their
//! `compat` modules.

use super::ntex;
pub use crate::common::VldBodyLimit;
use crate::common::{
    DEFAULT_BODY_LIMIT, VldHttpError, cookie_headers_to_json, headers_to_json, parse_coerced,
    parse_form_body, parse_json_body, path_params_to_json, query_string_to_json,
    require_form_content_type, require_json_content_type,
};
use ntex::http::{Payload, StatusCode, header};
use ntex::web::{HttpRequest, HttpResponse, HttpResponseBuilder};
use std::fmt;
use vld::schema::VldParse;

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
    pub(super) fn render(&self) -> HttpResponse {
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
    // With `ntex-compress`, decode `Content-Encoding` like ntex's own body
    // extractors; the limit applies to the decoded size.
    #[cfg(feature = "ntex-compress")]
    let mut stream = ntex::http::encoding::Decoder::from_headers(payload.take(), req.headers());
    #[cfg(not(feature = "ntex-compress"))]
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

// ============================= VldJson =======================================

/// ntex extractor that validates **JSON request bodies**.
///
/// Drop-in replacement for `ntex::web::types::Json<T>`.
pub struct VldJson<T>(pub T);

impl<T: VldParse> VldJson<T> {
    pub(super) async fn extract(
        req: &HttpRequest,
        payload: &mut Payload,
    ) -> Result<Self, VldNtexError> {
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
/// See [`crate::common::parse_query_string`] for coercion and array rules.
pub struct VldQuery<T>(pub T);

impl<T: VldParse> VldQuery<T> {
    pub(super) async fn extract(
        req: &HttpRequest,
        _payload: &mut Payload,
    ) -> Result<Self, VldNtexError> {
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
    pub(super) async fn extract(
        req: &HttpRequest,
        _payload: &mut Payload,
    ) -> Result<Self, VldNtexError> {
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
    pub(super) async fn extract(
        req: &HttpRequest,
        payload: &mut Payload,
    ) -> Result<Self, VldNtexError> {
        require_form_content_type(content_type(req))?;
        let body = read_body(req, payload).await?;
        validate(&parse_form_body(&body)?).map(VldForm)
    }
}

// ============================= VldHeaders ====================================

/// ntex extractor that validates **HTTP headers**.
///
/// See [`crate::common::headers_to_json`]: names become snake_case
/// (`X-Request-Id` → `x_request_id`), names containing `_` are ignored,
/// repeated headers are joined with `", "`.
pub struct VldHeaders<T>(pub T);

impl<T: VldParse> VldHeaders<T> {
    pub(super) async fn extract(
        req: &HttpRequest,
        _payload: &mut Payload,
    ) -> Result<Self, VldNtexError> {
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
    pub(super) async fn extract(
        req: &HttpRequest,
        _payload: &mut Payload,
    ) -> Result<Self, VldNtexError> {
        let value =
            cookie_headers_to_json(req.headers().get_all(header::COOKIE).map(|v| v.as_bytes()));
        validate(&value).map(VldCookie)
    }
}

crate::impl_deref!(VldJson, VldQuery, VldPath, VldForm, VldHeaders, VldCookie);

/// Prelude — import everything you need.
pub mod prelude {
    pub use super::{
        VldBodyLimit, VldCookie, VldForm, VldHeaders, VldJson, VldNtexError, VldPath, VldQuery,
    };
    pub use vld::prelude::*;
}
