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
//! | `VldPath<T>` | Path segments | `<param>` segments |
//! | `VldForm<T>` | Form body | `rocket::form::Form<T>` |
//! | `VldHeaders<T>` | HTTP headers | manual extraction |
//! | `VldCookie<T>` | Cookie values | `CookieJar` |
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
use vld_http_common::{
    coerce_value, cookies_to_json, format_vld_error, parse_query_string as parse_query_to_json,
};

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
                let body = vld_http_common::format_json_parse_error(&format!("{e}"));
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
                let body = vld_http_common::format_payload_too_large();
                store_error(req, body.clone());
                return DataOutcome::Error((Status::PayloadTooLarge, body));
            }
        };

        let body_str = match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => {
                let body = vld_http_common::format_utf8_error();
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
        .unwrap_or_else(|| vld_http_common::format_generic_error("Unprocessable Entity"));
    (Status::UnprocessableEntity, Json(body))
}

/// Catcher for `400 Bad Request` that returns JSON.
#[rocket::catch(400)]
pub fn vld_400_catcher(req: &Request<'_>) -> (Status, Json<serde_json::Value>) {
    let cached = req.local_cache(|| VldErrorCache(None));
    let body = cached
        .0
        .clone()
        .unwrap_or_else(|| vld_http_common::format_generic_error("Bad Request"));
    (Status::BadRequest, Json(body))
}

// ---------------------------------------------------------------------------
// VldPath<T> — validated path parameters
// ---------------------------------------------------------------------------

/// Validated path parameter extractor for Rocket.
///
/// Extracts named path segments and validates via `T::vld_parse_value()`.
/// Path values are coerced: `"42"` → number, `"true"` → bool, etc.
///
/// Use Rocket's `<param>` syntax to define path parameters.
/// The struct field names must match the parameter names.
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

#[rocket::async_trait]
impl<'r, T: VldParse + Send + Sync + 'static> FromRequest<'r> for VldPath<T> {
    type Error = serde_json::Value;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let mut map = serde_json::Map::new();

        // Rocket exposes route segments in match_info via routed_segments
        // We iterate over the raw segments from the uri
        for (i, seg) in req.routed_segments(0..).enumerate() {
            // Try to get param name from route if available
            let key = format!("{}", i);
            let _ = key; // fallback
            map.insert(seg.to_string(), coerce_value(seg));
        }

        // Better approach: use named query params from Rocket's param API
        // Since Rocket doesn't expose route pattern names easily,
        // we'll read all segments as positional values and also try
        // to extract by common names from the route's dynamic segments
        let mut named_map = serde_json::Map::new();

        // Extract each dynamic param by trying common names
        // Rocket stores route segments; we can access them by index
        let segments: Vec<&str> = req.routed_segments(0..).collect();
        if let Some(route) = req.route() {
            let uri_str = route.uri.origin.path().as_str();
            let mut param_idx = 0;
            for part in uri_str.split('/') {
                if part.starts_with('<') && part.ends_with('>') {
                    let name = part
                        .trim_start_matches('<')
                        .trim_end_matches('>')
                        .trim_end_matches("..");
                    if let Some(&seg_value) = segments.get(param_idx) {
                        named_map.insert(name.to_string(), coerce_value(seg_value));
                    }
                    param_idx += 1;
                } else if !part.is_empty() {
                    param_idx += 1;
                }
            }
        }

        let value = serde_json::Value::Object(named_map);

        match T::vld_parse_value(&value) {
            Ok(parsed) => Outcome::Success(VldPath(parsed)),
            Err(vld_err) => {
                let body = format_vld_error(&vld_err);
                store_error(req, body.clone());
                Outcome::Error((Status::UnprocessableEntity, body))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// VldHeaders<T> — validated HTTP headers
// ---------------------------------------------------------------------------

/// Validated HTTP headers extractor for Rocket.
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

#[rocket::async_trait]
impl<'r, T: VldParse + Send + Sync + 'static> FromRequest<'r> for VldHeaders<T> {
    type Error = serde_json::Value;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let mut map = serde_json::Map::new();

        for header in req.headers().iter() {
            let key = header.name().as_str().to_lowercase().replace('-', "_");
            map.insert(key, coerce_value(header.value()));
        }

        let value = serde_json::Value::Object(map);

        match T::vld_parse_value(&value) {
            Ok(parsed) => Outcome::Success(VldHeaders(parsed)),
            Err(vld_err) => {
                let body = format_vld_error(&vld_err);
                store_error(req, body.clone());
                Outcome::Error((Status::UnprocessableEntity, body))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// VldCookie<T> — validated cookies
// ---------------------------------------------------------------------------

/// Validated cookie extractor for Rocket.
///
/// Reads cookies from the `Cookie` header and validates against the schema.
/// Cookie names are used as-is for field matching.
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

#[rocket::async_trait]
impl<'r, T: VldParse + Send + Sync + 'static> FromRequest<'r> for VldCookie<T> {
    type Error = serde_json::Value;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let cookie_header = req.headers().get_one("Cookie").unwrap_or("");

        let value = cookies_to_json(cookie_header);

        match T::vld_parse_value(&value) {
            Ok(parsed) => Outcome::Success(VldCookie(parsed)),
            Err(vld_err) => {
                let body = format_vld_error(&vld_err);
                store_error(req, body.clone());
                Outcome::Error((Status::UnprocessableEntity, body))
            }
        }
    }
}

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{
        vld_400_catcher, vld_422_catcher, VldCookie, VldForm, VldHeaders, VldJson, VldPath,
        VldQuery,
    };
    pub use vld::prelude::*;
}
