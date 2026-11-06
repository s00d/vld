//! # vld-rocket — Rocket integration for `vld`
//!
//! Validation extractors for [Rocket](https://rocket.rs/). Validates request
//! data against `vld` schemas and returns `422 Unprocessable Entity` with
//! structured JSON errors on failure (malformed JSON/form bodies included),
//! and `413 Payload Too Large` when a body exceeds the limit.
//! `VldJson` requires a JSON `Content-Type` (`application/json` or
//! `application/*+json`) and `VldForm` `application/x-www-form-urlencoded`;
//! anything else is `415 Unsupported Media Type`. See the
//! [error body format](vld_http_common#error-bodies).
//!
//! Guide: <https://s00d.github.io/vld/integrations/web/rocket> (shared rules
//! for all adapters: <https://s00d.github.io/vld/integrations/web/overview>).
//!
//! # Example
//!
//! ```no_run
//! use vld_rocket::prelude::*;
//!
//! vld::schema! {
//!     #[derive(Debug, Clone)]
//!     pub struct CreateUser {
//!         pub name: String  => vld::string().min(2).max(50),
//!         pub email: String => vld::string().email(),
//!     }
//! }
//!
//! vld::schema! {
//!     #[derive(Debug, Clone)]
//!     pub struct UserPath {
//!         pub id: i64 => vld::number().int().min(1),
//!     }
//! }
//!
//! #[rocket::post("/users", data = "<user>")]
//! fn create_user(user: VldJson<CreateUser>) -> String {
//!     format!("created {}", user.name)
//! }
//!
//! // Rocket needs every `<param>` as an argument too; `VldPath` validates them.
//! #[rocket::get("/users/<id>")]
//! fn get_user(id: &str, p: VldPath<UserPath>) -> String {
//!     let _ = id;
//!     format!("user #{}", p.id)
//! }
//!
//! fn app() -> rocket::Rocket<rocket::Build> {
//!     rocket::build()
//!         .mount("/api", rocket::routes![create_user, get_user])
//!         .register("/", vld_catchers())
//! }
//! # fn main() { let _ = app(); }
//! ```
//!
//! # Extractors
//!
//! | Extractor | Source | Rocket equivalent |
//! |-----------|--------|-------------------|
//! | `VldJson<T>` | JSON body | `rocket::serde::json::Json<T>` |
//! | `VldQuery<T>` | Query string | query params |
//! | `VldPath<T>` | Path segments | `<param>` / `<param..>` segments |
//! | `VldForm<T>` | Form body | `rocket::form::Form<T>` |
//! | `VldHeaders<T>` | HTTP headers | manual extraction |
//! | `VldCookie<T>` | Cookie values | `CookieJar` |
//!
//! # Body limits
//!
//! `VldJson` reads up to Rocket's `json` limit and `VldForm` up to the
//! `form` limit, configured via `limits` in `Rocket.toml` (or
//! `ROCKET_LIMITS`). While a limit is left at Rocket's built-in default
//! (1 MiB / 32 KiB) [`DEFAULT_BODY_LIMIT`] (2 MiB) applies instead, like in
//! every other `vld` adapter — so setting exactly the built-in value counts
//! as unset.
//!
//! # Error catchers
//!
//! Register [`vld_catchers()`] (as in the example above) to get JSON error
//! responses instead of Rocket's default HTML pages. A failed guard only sets
//! the status; the catcher answers with the error body the guard stored.

use rocket::data::{ByteUnit, Data, FromData, Limits, Outcome as DataOutcome, ToByteUnit};
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use rocket::serde::json::Json;
use std::sync::Mutex;
use vld::schema::VldParse;
use vld_http_common::{
    cookie_headers_to_json, headers_to_json, parse_coerced, parse_form_body, parse_json_body,
    path_params_to_json, query_string_to_json, require_form_content_type,
    require_json_content_type, VldHttpError, DEFAULT_BODY_LIMIT,
};

// ---------------------------------------------------------------------------
// Request-local error storage
// ---------------------------------------------------------------------------

/// The latest vld failure `(status, body)` of a request, for the catchers.
#[derive(Default)]
struct VldErrorCache(Mutex<Option<(Status, serde_json::Value)>>);

/// Store the error body for the catchers and return `(status, body)`.
fn fail(req: &Request<'_>, err: VldHttpError) -> (Status, serde_json::Value) {
    let status = Status::new(err.status());
    let body = err.body();
    // A later failure overwrites: the catcher answers for the last one.
    *req.local_cache(VldErrorCache::default)
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some((status, body.clone()));
    (status, body)
}

/// Rocket's limit `name`, or [`DEFAULT_BODY_LIMIT`] while it is unset or
/// left at Rocket's built-in default.
fn body_limit(req: &Request<'_>, name: &str) -> ByteUnit {
    match req.limits().get(name) {
        Some(limit) if Some(limit) != Limits::default().get(name) => limit,
        _ => DEFAULT_BODY_LIMIT.bytes(),
    }
}

/// Read the body up to `limit`: 413 beyond it, 422 on a read error.
async fn read_body(
    req: &Request<'_>,
    data: Data<'_>,
    limit: ByteUnit,
) -> Result<Vec<u8>, (Status, serde_json::Value)> {
    match data.open(limit).into_bytes().await {
        Ok(b) if b.is_complete() => Ok(b.into_inner()),
        Ok(_) => Err(fail(req, VldHttpError::payload_too_large())),
        Err(e) => Err(fail(
            req,
            VldHttpError::parse(format!("Failed to read request body: {e}")),
        )),
    }
}

fn validate<T: VldParse>(
    req: &Request<'_>,
    value: &serde_json::Value,
) -> Result<T, (Status, serde_json::Value)> {
    parse_coerced(value).map_err(|e| fail(req, VldHttpError::validation(e)))
}

fn to_outcome<S>(r: Result<S, (Status, serde_json::Value)>) -> Outcome<S, serde_json::Value> {
    match r {
        Ok(v) => Outcome::Success(v),
        Err(e) => Outcome::Error(e),
    }
}

fn to_data_outcome<'r, S>(
    r: Result<S, (Status, serde_json::Value)>,
) -> DataOutcome<'r, S, serde_json::Value> {
    match r {
        Ok(v) => DataOutcome::Success(v),
        Err(e) => DataOutcome::Error(e),
    }
}

// ---------------------------------------------------------------------------
// VldJson<T> — validated JSON body
// ---------------------------------------------------------------------------

/// Validated JSON body extractor.
///
/// Reads the request body as JSON, validates via `T::vld_parse_value()`,
/// and returns `422` with error details on failure (`413` when the body
/// exceeds the `json` limit, `415` when the `Content-Type` is missing
/// or not `application/json` / `application/*+json`).
#[derive(Debug, Clone)]
pub struct VldJson<T>(pub T);

#[rocket::async_trait]
impl<'r, T: VldParse + Send + 'static> FromData<'r> for VldJson<T> {
    type Error = serde_json::Value;

    async fn from_data(req: &'r Request<'_>, data: Data<'r>) -> DataOutcome<'r, Self> {
        if let Err(e) = require_json_content_type(req.headers().get_one("Content-Type")) {
            return DataOutcome::Error(fail(req, e));
        }
        let result = match read_body(req, data, body_limit(req, "json")).await {
            Ok(body) => parse_json_body(&body)
                .and_then(|v| T::vld_parse_value(&v).map_err(VldHttpError::validation))
                .map(VldJson)
                .map_err(|e| fail(req, e)),
            Err(e) => Err(e),
        };
        to_data_outcome(result)
    }
}

// ---------------------------------------------------------------------------
// VldQuery<T> — validated query string
// ---------------------------------------------------------------------------

/// Validated query string extractor.
///
/// See [`vld_http_common::parse_query_string`] for coercion and array rules.
#[derive(Debug, Clone)]
pub struct VldQuery<T>(pub T);

#[rocket::async_trait]
impl<'r, T: VldParse + Send + Sync + 'static> FromRequest<'r> for VldQuery<T> {
    type Error = serde_json::Value;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let qs = req.uri().query().map(|q| q.as_str()).unwrap_or("");
        to_outcome(validate(req, &query_string_to_json(qs)).map(VldQuery))
    }
}

// ---------------------------------------------------------------------------
// VldForm<T> — validated form body
// ---------------------------------------------------------------------------

/// Validated form body extractor.
///
/// Reads an `application/x-www-form-urlencoded` body (`415` for another
/// `Content-Type`, `413` above the `form` limit), parses it like a query
/// string and validates via `T::vld_parse_value()`.
#[derive(Debug, Clone)]
pub struct VldForm<T>(pub T);

#[rocket::async_trait]
impl<'r, T: VldParse + Send + 'static> FromData<'r> for VldForm<T> {
    type Error = serde_json::Value;

    async fn from_data(req: &'r Request<'_>, data: Data<'r>) -> DataOutcome<'r, Self> {
        if let Err(e) = require_form_content_type(req.headers().get_one("Content-Type")) {
            return DataOutcome::Error(fail(req, e));
        }
        let result = match read_body(req, data, body_limit(req, "form")).await {
            Ok(body) => parse_form_body(&body)
                .map_err(|e| fail(req, e))
                .and_then(|value| validate(req, &value)),
            Err(e) => Err(e),
        };
        to_data_outcome(result.map(VldForm))
    }
}

// ---------------------------------------------------------------------------
// Error catchers
// ---------------------------------------------------------------------------

/// The stored vld error body when it is for `status`; otherwise (another
/// guard failed, e.g. Rocket's own `Json`) a generic `{"error": reason}`.
fn cached_or(req: &Request<'_>, status: Status) -> (Status, Json<serde_json::Value>) {
    let cached = req
        .local_cache(VldErrorCache::default)
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let body = match cached {
        Some((s, body)) if s == status => body,
        _ => vld_http_common::format_generic_error(status.reason_lossy()),
    };
    (status, Json(body))
}

/// All vld catchers: `400`, `413`, `415` and `422`.
///
/// ```rust,ignore
/// rocket::build().register("/", vld_rocket::vld_catchers())
/// ```
pub fn vld_catchers() -> Vec<rocket::Catcher> {
    rocket::catchers![
        vld_400_catcher,
        vld_413_catcher,
        vld_415_catcher,
        vld_422_catcher
    ]
}

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
    cached_or(req, Status::UnprocessableEntity)
}

/// Catcher for `400 Bad Request` that returns JSON.
#[rocket::catch(400)]
pub fn vld_400_catcher(req: &Request<'_>) -> (Status, Json<serde_json::Value>) {
    cached_or(req, Status::BadRequest)
}

/// Catcher for `413 Payload Too Large` that returns
/// `{"error": "Payload too large"}`.
#[rocket::catch(413)]
pub fn vld_413_catcher(_req: &Request<'_>) -> (Status, Json<serde_json::Value>) {
    (
        Status::PayloadTooLarge,
        Json(vld_http_common::format_payload_too_large()),
    )
}

/// Catcher for `415 Unsupported Media Type` that returns JSON.
#[rocket::catch(415)]
pub fn vld_415_catcher(req: &Request<'_>) -> (Status, Json<serde_json::Value>) {
    cached_or(req, Status::UnsupportedMediaType)
}

// ---------------------------------------------------------------------------
// VldPath<T> — validated path parameters
// ---------------------------------------------------------------------------

/// Validated path parameter extractor for Rocket.
///
/// Extracts named dynamic segments of the matched route (`<id>`; a trailing
/// `<path..>` receives the remaining segments joined with `/`) and validates
/// via `T::vld_parse_value()`. Works with any mount point.
///
/// The struct field names must match the parameter names.
#[derive(Debug, Clone)]
pub struct VldPath<T>(pub T);

/// Map the route's dynamic segments to the request's routed segments.
fn path_params(req: &Request<'_>) -> Vec<(String, String)> {
    let Some(route) = req.route() else {
        return Vec::new();
    };
    // `unmounted_origin` and `routed_segments` both exclude the mount base.
    let mut values = req.routed_segments(0..);
    let mut params = Vec::new();
    for pattern in route.uri.unmounted_origin.path().segments() {
        let value = values.next();
        let Some(name) = pattern.strip_prefix('<').and_then(|p| p.strip_suffix('>')) else {
            continue;
        };
        if let Some(name) = name.strip_suffix("..") {
            let rest: Vec<&str> = value.into_iter().chain(values.by_ref()).collect();
            params.push((name.to_string(), rest.join("/")));
            break;
        }
        if let Some(value) = value {
            params.push((name.to_string(), value.to_string()));
        }
    }
    params
}

#[rocket::async_trait]
impl<'r, T: VldParse + Send + Sync + 'static> FromRequest<'r> for VldPath<T> {
    type Error = serde_json::Value;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let params = path_params(req);
        let value = path_params_to_json(params.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        to_outcome(validate(req, &value).map(VldPath))
    }
}

// ---------------------------------------------------------------------------
// VldHeaders<T> — validated HTTP headers
// ---------------------------------------------------------------------------

/// Validated HTTP headers extractor for Rocket.
///
/// See [`vld_http_common::headers_to_json`]: names become snake_case
/// (`Content-Type` → `content_type`), names containing `_` are ignored,
/// repeated headers are joined with `", "`.
#[derive(Debug, Clone)]
pub struct VldHeaders<T>(pub T);

#[rocket::async_trait]
impl<'r, T: VldParse + Send + Sync + 'static> FromRequest<'r> for VldHeaders<T> {
    type Error = serde_json::Value;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        // `iter()` yields `Header`s by value; keep them alive while borrowed.
        let headers: Vec<_> = req.headers().iter().collect();
        let value = headers_to_json(
            headers
                .iter()
                .map(|h| (h.name().as_str(), h.value().as_bytes())),
        );
        to_outcome(validate(req, &value).map(VldHeaders))
    }
}

// ---------------------------------------------------------------------------
// VldCookie<T> — validated cookies
// ---------------------------------------------------------------------------

/// Validated cookie extractor for Rocket.
///
/// Reads cookies from every `Cookie` header (the first occurrence of a name
/// wins) and validates against the schema. Cookie names are used as-is;
/// values are unquoted, percent-decoded and coerced.
#[derive(Debug, Clone)]
pub struct VldCookie<T>(pub T);

#[rocket::async_trait]
impl<'r, T: VldParse + Send + Sync + 'static> FromRequest<'r> for VldCookie<T> {
    type Error = serde_json::Value;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let value = cookie_headers_to_json(req.headers().get("Cookie").map(str::as_bytes));
        to_outcome(validate(req, &value).map(VldCookie))
    }
}

vld_http_common::impl_deref!(VldJson, VldQuery, VldPath, VldForm, VldHeaders, VldCookie);

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{
        vld_400_catcher, vld_413_catcher, vld_415_catcher, vld_422_catcher, vld_catchers,
        VldCookie, VldForm, VldHeaders, VldJson, VldPath, VldQuery,
    };
    pub use vld::prelude::*;
}
