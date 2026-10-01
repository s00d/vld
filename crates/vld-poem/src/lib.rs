//! # vld-poem — Poem integration for `vld`
//!
//! Validation extractors for [Poem](https://docs.rs/poem). Validates request
//! data against `vld` schemas and returns `422 Unprocessable Entity` with
//! structured JSON errors on failure.
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
use std::ops::{Deref, DerefMut};
use vld::schema::VldParse;
use vld_http_common::{
    coerce_value, cookies_to_json, format_payload_too_large, format_utf8_error, format_vld_error,
    parse_query_string as parse_query_to_json, DEFAULT_BODY_LIMIT,
};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Validation / parse error returned by vld-poem extractors.
#[derive(Debug)]
pub struct VldPoemError {
    /// JSON response body.
    pub body: serde_json::Value,
    status: StatusCode,
}

impl VldPoemError {
    fn validation(err: vld::error::VldError) -> Self {
        Self {
            body: format_vld_error(&err),
            status: StatusCode::UNPROCESSABLE_ENTITY,
        }
    }

    fn parse(message: impl fmt::Display) -> Self {
        Self::validation(vld::error::VldError::single(
            vld::error::IssueCode::ParseError,
            message.to_string(),
        ))
    }

    fn payload_too_large() -> Self {
        Self {
            body: format_payload_too_large(),
            status: StatusCode::PAYLOAD_TOO_LARGE,
        }
    }

    fn utf8() -> Self {
        Self {
            body: format_utf8_error(),
            status: StatusCode::UNPROCESSABLE_ENTITY,
        }
    }
}

impl fmt::Display for VldPoemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.body)
    }
}

impl std::error::Error for VldPoemError {}

impl ResponseError for VldPoemError {
    fn status(&self) -> StatusCode {
        self.status
    }

    fn as_response(&self) -> poem::Response {
        poem::Response::builder()
            .status(self.status)
            .content_type("application/json")
            .body(serde_json::to_string(&self.body).unwrap_or_default())
    }
}

async fn read_limited_body(body: &mut RequestBody) -> Result<Vec<u8>> {
    let bytes = body
        .take()?
        .into_bytes_limit(DEFAULT_BODY_LIMIT)
        .await
        .map_err(|e| match e {
            ReadBodyError::PayloadTooLarge => poem::Error::from(VldPoemError::payload_too_large()),
            other => poem::Error::from(other),
        })?;
    Ok(bytes.to_vec())
}

// ---------------------------------------------------------------------------
// VldJson<T>
// ---------------------------------------------------------------------------

/// Validated JSON body extractor for Poem.
#[derive(Debug, Clone)]
pub struct VldJson<T>(pub T);

impl<T> Deref for VldJson<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for VldJson<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<'a, T: VldParse + Send + Sync + 'static> FromRequest<'a> for VldJson<T> {
    async fn from_request(_req: &'a Request, body: &mut RequestBody) -> Result<Self> {
        let bytes = read_limited_body(body).await?;
        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| VldPoemError::parse(format!("Invalid JSON: {e}")))?;

        T::vld_parse_value(&value)
            .map(VldJson)
            .map_err(|e| VldPoemError::validation(e).into())
    }
}

// ---------------------------------------------------------------------------
// VldQuery<T>
// ---------------------------------------------------------------------------

/// Validated query string extractor for Poem.
#[derive(Debug, Clone)]
pub struct VldQuery<T>(pub T);

impl<T> Deref for VldQuery<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for VldQuery<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<'a, T: VldParse + Send + Sync + 'static> FromRequest<'a> for VldQuery<T> {
    async fn from_request(req: &'a Request, _body: &mut RequestBody) -> Result<Self> {
        let qs = req.uri().query().unwrap_or("");
        let map = parse_query_to_json(qs);
        let value = serde_json::Value::Object(map);

        T::vld_parse_value(&value)
            .map(VldQuery)
            .map_err(|e| VldPoemError::validation(e).into())
    }
}

// ---------------------------------------------------------------------------
// VldForm<T>
// ---------------------------------------------------------------------------

/// Validated form body extractor for Poem.
#[derive(Debug, Clone)]
pub struct VldForm<T>(pub T);

impl<T> Deref for VldForm<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for VldForm<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<'a, T: VldParse + Send + Sync + 'static> FromRequest<'a> for VldForm<T> {
    async fn from_request(_req: &'a Request, body: &mut RequestBody) -> Result<Self> {
        let bytes = read_limited_body(body).await?;
        let body_str = String::from_utf8(bytes.to_vec()).map_err(|_| VldPoemError::utf8())?;

        let map = parse_query_to_json(&body_str);
        let value = serde_json::Value::Object(map);

        T::vld_parse_value(&value)
            .map(VldForm)
            .map_err(|e| VldPoemError::validation(e).into())
    }
}

// ---------------------------------------------------------------------------
// VldPath<T>
// ---------------------------------------------------------------------------

/// Validated path parameters extractor for Poem.
///
/// Path values are coerced: `"42"` → number, `"true"` → bool, etc.
#[derive(Debug, Clone)]
pub struct VldPath<T>(pub T);

impl<T> Deref for VldPath<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for VldPath<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<'a, T: VldParse + Send + Sync + 'static> FromRequest<'a> for VldPath<T> {
    async fn from_request(req: &'a Request, _body: &mut RequestBody) -> Result<Self> {
        let params = req.params::<Vec<(String, String)>>().unwrap_or_default();

        let mut map = serde_json::Map::new();
        for (k, v) in &params {
            map.insert(k.clone(), coerce_value(v));
        }
        let value = serde_json::Value::Object(map);

        T::vld_parse_value(&value)
            .map(VldPath)
            .map_err(|e| VldPoemError::validation(e).into())
    }
}

// ---------------------------------------------------------------------------
// VldHeaders<T>
// ---------------------------------------------------------------------------

/// Validated HTTP headers extractor for Poem.
///
/// Header names are normalised to snake_case: `Content-Type` → `content_type`.
/// Values are coerced: `"42"` → number, `"true"` → bool, etc.
#[derive(Debug, Clone)]
pub struct VldHeaders<T>(pub T);

impl<T> Deref for VldHeaders<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for VldHeaders<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<'a, T: VldParse + Send + Sync + 'static> FromRequest<'a> for VldHeaders<T> {
    async fn from_request(req: &'a Request, _body: &mut RequestBody) -> Result<Self> {
        let mut map = serde_json::Map::new();
        for (name, value) in req.headers().iter() {
            let key = name.as_str().to_lowercase().replace('-', "_");
            if let Ok(v) = value.to_str() {
                map.insert(key, coerce_value(v));
            }
        }
        let value = serde_json::Value::Object(map);

        T::vld_parse_value(&value)
            .map(VldHeaders)
            .map_err(|e| VldPoemError::validation(e).into())
    }
}

// ---------------------------------------------------------------------------
// VldCookie<T>
// ---------------------------------------------------------------------------

/// Validated cookie extractor for Poem.
///
/// Reads cookies from the `Cookie` header and validates against the schema.
#[derive(Debug, Clone)]
pub struct VldCookie<T>(pub T);

impl<T> Deref for VldCookie<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for VldCookie<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<'a, T: VldParse + Send + Sync + 'static> FromRequest<'a> for VldCookie<T> {
    async fn from_request(req: &'a Request, _body: &mut RequestBody) -> Result<Self> {
        let cookie_header = req
            .headers()
            .get("cookie")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let value = cookies_to_json(cookie_header);

        T::vld_parse_value(&value)
            .map(VldCookie)
            .map_err(|e| VldPoemError::validation(e).into())
    }
}

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{VldCookie, VldForm, VldHeaders, VldJson, VldPath, VldQuery};
    pub use vld::prelude::*;
}
