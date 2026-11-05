//! # vld-poem — Poem integration for `vld`
//!
//! Validation extractors for [Poem](https://docs.rs/poem). Validates request
//! data against `vld` schemas and returns `422 Unprocessable Entity` with
//! structured JSON errors on failure (`413 Payload Too Large` when a body
//! exceeds the limit). `VldJson` requires a JSON `Content-Type`
//! (`application/json` or `application/*+json`) and `VldForm`
//! `application/x-www-form-urlencoded`; anything else is
//! `415 Unsupported Media Type`. See the
//! [error body format](vld_http_common#error-bodies).
//!
//! # Body limit
//!
//! `VldJson` and `VldForm` read at most [`DEFAULT_BODY_LIMIT`] (2 MiB);
//! attach [`VldBodyLimit`] as request data to change it:
//!
//! ```ignore
//! Route::new().at("/upload", post(handler)).data(vld_poem::VldBodyLimit(8 << 20))
//! ```
//!
//! # Extractors
//!
//! | Extractor | Source |
//! |-----------|--------|
//! | `VldJson<T>` | JSON body |
//! | `VldQuery<T>` | Query string |
//! | `VldPath<T>` | Path parameters |
//! | `VldForm<T>` | Form body |
//! | `VldHeaders<T>` | HTTP headers |
//! | `VldCookie<T>` | Cookie values |

use poem::error::{ReadBodyError, ResponseError};
use poem::http::StatusCode;
use poem::{FromRequest, Request, RequestBody, Result};
use std::fmt;
use vld::schema::VldParse;
pub use vld_http_common::VldBodyLimit;
use vld_http_common::{
    cookie_headers_to_json, headers_to_json, parse_form_body, parse_json_body,
    path_params_to_json, query_string_to_json, require_form_content_type,
    require_json_content_type, VldHttpError, DEFAULT_BODY_LIMIT,
};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Validation / parse error returned by vld-poem extractors; wraps
/// [`VldHttpError`].
#[derive(Debug)]
pub struct VldPoemError {
    inner: VldHttpError,
}

impl VldPoemError {
    /// The underlying validation error.
    pub fn error(&self) -> &vld::error::VldError {
        self.inner.error()
    }

    /// HTTP status (422, 413 or 415).
    pub fn status(&self) -> StatusCode {
        StatusCode::from_u16(self.inner.status()).unwrap_or(StatusCode::UNPROCESSABLE_ENTITY)
    }

    /// JSON response body.
    pub fn body(&self) -> serde_json::Value {
        self.inner.body()
    }
}

impl From<VldHttpError> for VldPoemError {
    fn from(inner: VldHttpError) -> Self {
        Self { inner }
    }
}

impl From<vld::error::VldError> for VldPoemError {
    fn from(e: vld::error::VldError) -> Self {
        VldHttpError::from(e).into()
    }
}

impl fmt::Display for VldPoemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl std::error::Error for VldPoemError {}

impl ResponseError for VldPoemError {
    fn status(&self) -> StatusCode {
        VldPoemError::status(self)
    }

    fn as_response(&self) -> poem::Response {
        poem::Response::builder()
            .status(VldPoemError::status(self))
            .content_type("application/json")
            .body(self.body().to_string())
    }
}

fn reject(e: VldHttpError) -> poem::Error {
    VldPoemError::from(e).into()
}

/// Validate a value coerced from text (query, path, form, headers, cookies).
fn validate<T: VldParse>(value: &serde_json::Value) -> Result<T> {
    vld_http_common::parse_coerced(value).map_err(|e| VldPoemError::from(e).into())
}

/// Read the body up to the [`VldBodyLimit`] request data, or
/// [`DEFAULT_BODY_LIMIT`].
async fn read_limited_body(req: &Request, body: &mut RequestBody) -> Result<impl AsRef<[u8]>> {
    let limit = req
        .data::<VldBodyLimit>()
        .map_or(DEFAULT_BODY_LIMIT, |l| l.0);
    let body = body
        .take()
        .map_err(|e| reject(VldHttpError::parse(format!("Failed to read body: {e}"))))?;
    body.into_bytes_limit(limit)
        .await
        .map_err(|e| match e {
            ReadBodyError::PayloadTooLarge => reject(VldHttpError::payload_too_large()),
            other => reject(VldHttpError::parse(format!("Failed to read body: {other}"))),
        })
}

// ---------------------------------------------------------------------------
// VldJson<T>
// ---------------------------------------------------------------------------

/// Validated JSON body extractor for Poem.
#[derive(Debug, Clone)]
pub struct VldJson<T>(pub T);

impl<'a, T: VldParse + Send + Sync + 'static> FromRequest<'a> for VldJson<T> {
    async fn from_request(req: &'a Request, body: &mut RequestBody) -> Result<Self> {
        require_json_content_type(req.content_type()).map_err(reject)?;
        let bytes = read_limited_body(req, body).await?;
        T::vld_parse_value(&parse_json_body(bytes.as_ref()).map_err(reject)?)
            .map(VldJson)
            .map_err(|e| VldPoemError::from(e).into())
    }
}

// ---------------------------------------------------------------------------
// VldQuery<T>
// ---------------------------------------------------------------------------

/// Validated query string extractor for Poem.
///
/// See [`vld_http_common::parse_query_string`] for coercion and array rules.
#[derive(Debug, Clone)]
pub struct VldQuery<T>(pub T);

impl<'a, T: VldParse + Send + Sync + 'static> FromRequest<'a> for VldQuery<T> {
    async fn from_request(req: &'a Request, _body: &mut RequestBody) -> Result<Self> {
        validate(&query_string_to_json(req.uri().query().unwrap_or(""))).map(VldQuery)
    }
}

// ---------------------------------------------------------------------------
// VldForm<T>
// ---------------------------------------------------------------------------

/// Validated form body extractor for Poem.
#[derive(Debug, Clone)]
pub struct VldForm<T>(pub T);

impl<'a, T: VldParse + Send + Sync + 'static> FromRequest<'a> for VldForm<T> {
    async fn from_request(req: &'a Request, body: &mut RequestBody) -> Result<Self> {
        require_form_content_type(req.content_type()).map_err(reject)?;
        let bytes = read_limited_body(req, body).await?;
        validate(&parse_form_body(bytes.as_ref()).map_err(reject)?).map(VldForm)
    }
}

// ---------------------------------------------------------------------------
// VldPath<T>
// ---------------------------------------------------------------------------

/// Validated path parameters extractor for Poem.
///
/// Path values are coerced the same way as query parameters.
#[derive(Debug, Clone)]
pub struct VldPath<T>(pub T);

impl<'a, T: VldParse + Send + Sync + 'static> FromRequest<'a> for VldPath<T> {
    async fn from_request(req: &'a Request, _body: &mut RequestBody) -> Result<Self> {
        // `Request::params` parses the *query string*; route parameters come from
        // `path_params` (the router's match params).
        let params = req
            .path_params::<std::collections::BTreeMap<String, String>>()
            .unwrap_or_default();
        validate(&path_params_to_json(
            params.iter().map(|(k, v)| (k.as_str(), v.as_str())),
        ))
        .map(VldPath)
    }
}

// ---------------------------------------------------------------------------
// VldHeaders<T>
// ---------------------------------------------------------------------------

/// Validated HTTP headers extractor for Poem.
///
/// See [`vld_http_common::headers_to_json`]: names become snake_case
/// (`Content-Type` → `content_type`), names containing `_` are ignored,
/// repeated headers are joined with `", "`.
#[derive(Debug, Clone)]
pub struct VldHeaders<T>(pub T);

impl<'a, T: VldParse + Send + Sync + 'static> FromRequest<'a> for VldHeaders<T> {
    async fn from_request(req: &'a Request, _body: &mut RequestBody) -> Result<Self> {
        let value = headers_to_json(
            req.headers()
                .iter()
                .map(|(n, v)| (n.as_str(), v.as_bytes())),
        );
        validate(&value).map(VldHeaders)
    }
}

// ---------------------------------------------------------------------------
// VldCookie<T>
// ---------------------------------------------------------------------------

/// Validated cookie extractor for Poem.
///
/// Reads cookies from every `Cookie` header (the first occurrence of a name
/// wins) and validates against the schema. Values are unquoted,
/// percent-decoded and coerced.
#[derive(Debug, Clone)]
pub struct VldCookie<T>(pub T);

impl<'a, T: VldParse + Send + Sync + 'static> FromRequest<'a> for VldCookie<T> {
    async fn from_request(req: &'a Request, _body: &mut RequestBody) -> Result<Self> {
        let value = cookie_headers_to_json(
            req.headers()
                .get_all(poem::http::header::COOKIE)
                .iter()
                .map(|v| v.as_bytes()),
        );
        validate(&value).map(VldCookie)
    }
}

vld_http_common::impl_deref!(VldJson, VldQuery, VldPath, VldForm, VldHeaders, VldCookie);

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{
        VldBodyLimit, VldCookie, VldForm, VldHeaders, VldJson, VldPath, VldPoemError, VldQuery,
    };
    pub use vld::prelude::*;
}
