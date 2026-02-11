//! # vld-warp — Warp integration for `vld`
//!
//! Validation [filters](warp::Filter) for [Warp](https://docs.rs/warp).
//! Validates request data against `vld` schemas and rejects with structured
//! JSON errors on failure.
//!
//! # Filters
//!
//! | Filter | Source |
//! |--------|--------|
//! | `vld_json::<T>()` | JSON body |
//! | `vld_query::<T>()` | Query string |

use std::convert::Infallible;
use vld::schema::VldParse;
use warp::http::StatusCode;
use warp::reject::Reject;
use warp::{Filter, Rejection, Reply};

// ---------------------------------------------------------------------------
// Rejection types
// ---------------------------------------------------------------------------

/// Rejection when JSON parsing fails.
#[derive(Debug)]
pub struct InvalidJson {
    pub message: String,
}
impl Reject for InvalidJson {}

/// Rejection when vld validation fails.
#[derive(Debug)]
pub struct ValidationFailed {
    pub error: vld::error::VldError,
}
impl Reject for ValidationFailed {}

// ---------------------------------------------------------------------------
// vld_json filter
// ---------------------------------------------------------------------------

/// Warp filter that extracts and validates a JSON body.
///
/// Returns the validated `T` or rejects with [`ValidationFailed`].
///
/// # Example
///
/// ```rust,ignore
/// use vld_warp::vld_json;
///
/// vld::schema! {
///     #[derive(Debug, Clone)]
///     pub struct CreateUser {
///         pub name: String  => vld::string().min(2),
///         pub email: String => vld::string().email(),
///     }
/// }
///
/// let route = warp::post()
///     .and(warp::path("users"))
///     .and(vld_json::<CreateUser>())
///     .map(|user: CreateUser| {
///         warp::reply::json(&serde_json::json!({"name": user.name}))
///     });
/// ```
pub fn vld_json<T: VldParse + Send + 'static>(
) -> impl Filter<Extract = (T,), Error = Rejection> + Clone {
    warp::body::bytes().and_then(|bytes: bytes::Bytes| async move {
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| {
            warp::reject::custom(InvalidJson {
                message: e.to_string(),
            })
        })?;

        T::vld_parse_value(&value).map_err(|e| warp::reject::custom(ValidationFailed { error: e }))
    })
}

// ---------------------------------------------------------------------------
// vld_query filter
// ---------------------------------------------------------------------------

/// Warp filter that extracts and validates query parameters.
///
/// Parses the query string into a JSON object with value coercion,
/// then validates via `T::vld_parse_value()`.
///
/// # Example
///
/// ```rust,ignore
/// use vld_warp::vld_query;
///
/// vld::schema! {
///     #[derive(Debug, Clone)]
///     pub struct Pagination {
///         pub page: i64  => vld::number().int().min(1),
///         pub limit: i64 => vld::number().int().min(1).max(100),
///     }
/// }
///
/// let route = warp::get()
///     .and(warp::path("items"))
///     .and(vld_query::<Pagination>())
///     .map(|p: Pagination| {
///         warp::reply::json(&serde_json::json!({"page": p.page, "limit": p.limit}))
///     });
/// ```
pub fn vld_query<T: VldParse + Send + 'static>(
) -> impl Filter<Extract = (T,), Error = Rejection> + Clone {
    warp::query::raw()
        .or(warp::any().map(String::new))
        .unify()
        .and_then(|qs: String| async move {
            let map = parse_query_to_json(&qs);
            let value = serde_json::Value::Object(map);
            T::vld_parse_value(&value)
                .map_err(|e| warp::reject::custom(ValidationFailed { error: e }))
        })
}

// ---------------------------------------------------------------------------
// Recovery handler
// ---------------------------------------------------------------------------

/// Recovery handler that converts vld rejections into JSON responses.
///
/// Use with `warp::Filter::recover()`:
///
/// ```rust,ignore
/// use vld_warp::handle_rejection;
///
/// let routes = warp::any()
///     // ...your routes...
///     .recover(handle_rejection);
/// ```
pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    if let Some(e) = err.find::<ValidationFailed>() {
        let issues: Vec<serde_json::Value> = e
            .error
            .issues
            .iter()
            .map(|i| {
                serde_json::json!({
                    "path": i.path.iter().map(|p| p.to_string()).collect::<Vec<_>>().join("."),
                    "message": i.message,
                })
            })
            .collect();
        let body = serde_json::json!({"error": "Validation failed", "issues": issues});
        let reply =
            warp::reply::with_status(warp::reply::json(&body), StatusCode::UNPROCESSABLE_ENTITY);
        return Ok(reply);
    }

    if let Some(e) = err.find::<InvalidJson>() {
        let body = serde_json::json!({"error": "Invalid JSON", "message": e.message});
        let reply = warp::reply::with_status(warp::reply::json(&body), StatusCode::BAD_REQUEST);
        return Ok(reply);
    }

    let body = serde_json::json!({"error": "Not Found"});
    let reply = warp::reply::with_status(warp::reply::json(&body), StatusCode::NOT_FOUND);
    Ok(reply)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

use vld_http_common::parse_query_string as parse_query_to_json;

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{handle_rejection, vld_json, vld_query, InvalidJson, ValidationFailed};
    pub use vld::prelude::*;
}
