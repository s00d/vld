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
//! | `VldForm<T>` | Form body |

use poem::error::ResponseError;
use poem::http::StatusCode;
use poem::{FromRequest, Request, RequestBody, Result};
use std::fmt;
use std::ops::{Deref, DerefMut};
use vld::schema::VldParse;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Validation error returned by vld-poem extractors.
#[derive(Debug)]
pub struct VldPoemError(pub serde_json::Value);

impl fmt::Display for VldPoemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for VldPoemError {}

impl ResponseError for VldPoemError {
    fn status(&self) -> StatusCode {
        StatusCode::UNPROCESSABLE_ENTITY
    }

    fn as_response(&self) -> poem::Response {
        poem::Response::builder()
            .status(StatusCode::UNPROCESSABLE_ENTITY)
            .content_type("application/json")
            .body(serde_json::to_string(&self.0).unwrap_or_default())
    }
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
        let bytes = body.take()?.into_bytes().await?;
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| {
            VldPoemError(serde_json::json!({
                "error": "Invalid JSON",
                "message": e.to_string(),
            }))
        })?;

        T::vld_parse_value(&value).map(VldJson).map_err(|e| {
            let issues = format_issues(&e);
            VldPoemError(serde_json::json!({"error": "Validation failed", "issues": issues})).into()
        })
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

        T::vld_parse_value(&value).map(VldQuery).map_err(|e| {
            let issues = format_issues(&e);
            VldPoemError(serde_json::json!({"error": "Validation failed", "issues": issues})).into()
        })
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
        let bytes = body.take()?.into_bytes().await?;
        let body_str = String::from_utf8(bytes.to_vec())
            .map_err(|_| VldPoemError(serde_json::json!({"error": "Invalid UTF-8"})))?;

        let map = parse_query_to_json(&body_str);
        let value = serde_json::Value::Object(map);

        T::vld_parse_value(&value).map(VldForm).map_err(|e| {
            let issues = format_issues(&e);
            VldPoemError(serde_json::json!({"error": "Validation failed", "issues": issues})).into()
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

use vld_http_common::{format_issues, parse_query_string as parse_query_to_json};

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{VldForm, VldJson, VldQuery};
    pub use vld::prelude::*;
}
