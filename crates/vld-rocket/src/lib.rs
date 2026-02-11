//! # vld-rocket — Rocket integration for `vld`
//!
//! Validation extractors for [Rocket](https://rocket.rs/). Validates request
//! data against `vld` schemas and returns `422 Unprocessable Entity` with
//! structured JSON errors on failure.
//!
//! # Extractors
//!
//! | Extractor | Source | Rocket equivalent |
//! |-----------|--------|-------------------|
//! | `VldJson<T>` | JSON body | `rocket::serde::json::Json<T>` |
//! | `VldQuery<T>` | Query string | query params |
//! | `VldForm<T>` | Form body | `rocket::form::Form<T>` |
//!
//! # Error catcher
//!
//! Register [`vld_catcher()`] to get JSON error responses instead of the
//! default HTML:
//!
//! ```rust,ignore
//! rocket::build()
//!     .mount("/", routes![...])
//!     .register("/", catchers![vld_rocket::vld_422_catcher])
//! ```

use rocket::data::{Data, FromData, Outcome as DataOutcome};
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use rocket::serde::json::Json;
use std::ops::{Deref, DerefMut};
use vld::schema::VldParse;

// ---------------------------------------------------------------------------
// Request-local error storage
// ---------------------------------------------------------------------------

/// Stored in request local cache so the catcher can read it.
#[derive(Debug, Clone, Default)]
pub struct VldErrorCache(pub Option<serde_json::Value>);

fn store_error(req: &Request<'_>, err: serde_json::Value) {
    // Rocket's local_cache returns &T, setting a value requires the closure pattern
    let _ = req.local_cache(|| VldErrorCache(Some(err.clone())));
}

// ---------------------------------------------------------------------------
// VldJson<T> — validated JSON body
// ---------------------------------------------------------------------------

/// Validated JSON body extractor.
///
/// Reads the request body as JSON, validates via `T::vld_parse_value()`,
/// and returns `422` with error details on failure.
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

#[rocket::async_trait]
impl<'r, T: VldParse + Send + 'static> FromData<'r> for VldJson<T> {
    type Error = serde_json::Value;

    async fn from_data(req: &'r Request<'_>, data: Data<'r>) -> DataOutcome<'r, Self> {
        let json_outcome = <Json<serde_json::Value> as FromData<'r>>::from_data(req, data).await;
        let value = match json_outcome {
            DataOutcome::Success(json) => json.into_inner(),
            DataOutcome::Error((status, e)) => {
                let body = serde_json::json!({
                    "error": "Invalid JSON",
                    "message": format!("{e}"),
                });
                store_error(req, body.clone());
                return DataOutcome::Error((status, body));
            }
            DataOutcome::Forward(f) => return DataOutcome::Forward(f),
        };

        match T::vld_parse_value(&value) {
            Ok(parsed) => DataOutcome::Success(VldJson(parsed)),
            Err(vld_err) => {
                let body = format_vld_error(&vld_err);
                store_error(req, body.clone());
                DataOutcome::Error((Status::UnprocessableEntity, body))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// VldQuery<T> — validated query string
// ---------------------------------------------------------------------------

/// Validated query string extractor.
///
/// Parses query parameters into a JSON object (coercing string values),
/// validates via `T::vld_parse_value()`.
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

#[rocket::async_trait]
impl<'r, T: VldParse + Send + Sync + 'static> FromRequest<'r> for VldQuery<T> {
    type Error = serde_json::Value;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let qs = req.uri().query().map(|q| q.as_str()).unwrap_or("");
        let map = parse_query_to_json(qs);
        let value = serde_json::Value::Object(map);

        match T::vld_parse_value(&value) {
            Ok(parsed) => Outcome::Success(VldQuery(parsed)),
            Err(vld_err) => {
                let body = format_vld_error(&vld_err);
                store_error(req, body.clone());
                Outcome::Error((Status::UnprocessableEntity, body))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// VldForm<T> — validated form body
// ---------------------------------------------------------------------------

/// Validated form body extractor.
///
/// Reads `application/x-www-form-urlencoded` body, parses into a JSON object
/// (coercing values), and validates via `T::vld_parse_value()`.
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

#[rocket::async_trait]
impl<'r, T: VldParse + Send + 'static> FromData<'r> for VldForm<T> {
    type Error = serde_json::Value;

    async fn from_data(req: &'r Request<'_>, data: Data<'r>) -> DataOutcome<'r, Self> {
        use rocket::data::ToByteUnit;
        let bytes = match data.open(1.mebibytes()).into_bytes().await {
            Ok(b) if b.is_complete() => b.into_inner(),
            _ => {
                let body = serde_json::json!({"error": "Payload too large"});
                store_error(req, body.clone());
                return DataOutcome::Error((Status::PayloadTooLarge, body));
            }
        };

        let body_str = match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => {
                let body = serde_json::json!({"error": "Invalid UTF-8"});
                store_error(req, body.clone());
                return DataOutcome::Error((Status::BadRequest, body));
            }
        };

        let map = parse_query_to_json(&body_str);
        let value = serde_json::Value::Object(map);

        match T::vld_parse_value(&value) {
            Ok(parsed) => DataOutcome::Success(VldForm(parsed)),
            Err(vld_err) => {
                let body = format_vld_error(&vld_err);
                store_error(req, body.clone());
                DataOutcome::Error((Status::UnprocessableEntity, body))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Error catcher
// ---------------------------------------------------------------------------

/// Catcher for `422 Unprocessable Entity` that returns JSON from the
/// validation error stored by vld extractors.
///
/// Register in your Rocket application:
///
/// ```rust,ignore
/// rocket::build()
///     .register("/", catchers![vld_rocket::vld_422_catcher])
/// ```
#[rocket::catch(422)]
pub fn vld_422_catcher(req: &Request<'_>) -> (Status, Json<serde_json::Value>) {
    let cached = req.local_cache(|| VldErrorCache(None));
    let body = cached
        .0
        .clone()
        .unwrap_or_else(|| serde_json::json!({"error": "Unprocessable Entity"}));
    (Status::UnprocessableEntity, Json(body))
}

/// Catcher for `400 Bad Request` that returns JSON.
#[rocket::catch(400)]
pub fn vld_400_catcher(req: &Request<'_>) -> (Status, Json<serde_json::Value>) {
    let cached = req.local_cache(|| VldErrorCache(None));
    let body = cached
        .0
        .clone()
        .unwrap_or_else(|| serde_json::json!({"error": "Bad Request"}));
    (Status::BadRequest, Json(body))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

use vld_http_common::{format_vld_error, parse_query_string as parse_query_to_json};

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{vld_400_catcher, vld_422_catcher, VldForm, VldJson, VldQuery};
    pub use vld::prelude::*;
}
