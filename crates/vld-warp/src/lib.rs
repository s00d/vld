//! # vld-warp — Warp integration for `vld`
//!
//! Validation [filters](warp::Filter) for [Warp](https://docs.rs/warp).
//! Validates request data against `vld` schemas and rejects with structured
//! JSON errors on failure.
//!
//! # Filters
//!
//! | Filter / Function | Source |
//! |-------------------|--------|
//! | [`vld_json::<T>()`] | JSON body |
//! | [`vld_query::<T>()`] | Query string |
//! | [`vld_form::<T>()`] | URL-encoded form body |
//! | [`vld_param::<T>(name)`] | Single path segment |
//! | [`vld_path::<T>(names)`] | All remaining path segments (tail) |
//! | [`validate_path_params::<T>(pairs)`] | Pre-extracted path params |
//! | [`vld_headers::<T>()`] | HTTP headers |
//! | [`vld_cookie::<T>()`] | Cookie values |

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
        let body = vld_http_common::format_vld_error(&e.error);
        let reply =
            warp::reply::with_status(warp::reply::json(&body), StatusCode::UNPROCESSABLE_ENTITY);
        return Ok(reply);
    }

    if let Some(e) = err.find::<InvalidJson>() {
        let body = vld_http_common::format_json_parse_error(&e.message);
        let reply = warp::reply::with_status(warp::reply::json(&body), StatusCode::BAD_REQUEST);
        return Ok(reply);
    }

    let body = vld_http_common::format_generic_error("Not Found");
    let reply = warp::reply::with_status(warp::reply::json(&body), StatusCode::NOT_FOUND);
    Ok(reply)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

use vld_http_common::{coerce_value, cookies_to_json, parse_query_string as parse_query_to_json};

// ---------------------------------------------------------------------------
// vld_form filter
// ---------------------------------------------------------------------------

/// Warp filter that extracts and validates a URL-encoded form body.
///
/// Values are coerced: `"42"` → number, `"true"` → bool, empty → null.
pub fn vld_form<T: VldParse + Send + 'static>(
) -> impl Filter<Extract = (T,), Error = Rejection> + Clone {
    warp::body::bytes().and_then(|bytes: bytes::Bytes| async move {
        let body_str = std::str::from_utf8(&bytes).map_err(|_| {
            warp::reject::custom(InvalidJson {
                message: "Form body is not valid UTF-8".into(),
            })
        })?;

        let map = parse_query_to_json(body_str);
        let value = serde_json::Value::Object(map);

        T::vld_parse_value(&value).map_err(|e| warp::reject::custom(ValidationFailed { error: e }))
    })
}

// ---------------------------------------------------------------------------
// vld_param — single path parameter filter
// ---------------------------------------------------------------------------

/// Warp filter that extracts and validates **a single path segment**.
///
/// Works like `warp::path::param::<String>()` but coerces the raw string
/// value (numbers, booleans, null) and validates via `T::vld_parse_value()`.
///
/// The extracted segment is wrapped into a JSON object `{ "<name>": <coerced> }`
/// so that the vld schema field name matches.
///
/// # Example
///
/// ```rust,ignore
/// use vld_warp::vld_param;
///
/// vld::schema! {
///     #[derive(Debug, Clone)]
///     pub struct UserId {
///         pub id: i64 => vld::number().int().min(1),
///     }
/// }
///
/// // GET /users/<id>
/// let route = warp::path("users")
///     .and(vld_param::<UserId>("id"))
///     .and(warp::path::end())
///     .map(|p: UserId| {
///         warp::reply::json(&serde_json::json!({"id": p.id}))
///     });
/// ```
pub fn vld_param<T: VldParse + Send + 'static>(
    name: &'static str,
) -> impl Filter<Extract = (T,), Error = Rejection> + Clone {
    warp::path::param::<String>().and_then(move |raw: String| async move {
        let mut map = serde_json::Map::new();
        map.insert(name.to_string(), coerce_value(&raw));
        let value = serde_json::Value::Object(map);
        T::vld_parse_value(&value).map_err(|e| warp::reject::custom(ValidationFailed { error: e }))
    })
}

// ---------------------------------------------------------------------------
// vld_path — multi-param tail filter
// ---------------------------------------------------------------------------

/// Warp filter that extracts and validates **all remaining path segments**.
///
/// Uses `warp::path::tail()` internally — all segments after the current
/// position are consumed and mapped to `param_names` in order.
/// The number of remaining segments **must equal** the number of names;
/// otherwise the request is rejected with 404 (not found).
///
/// Best suited for routes where all remaining segments are parameters
/// (no static segments left).
///
/// # Example
///
/// ```rust,ignore
/// use vld_warp::vld_path;
///
/// vld::schema! {
///     #[derive(Debug, Clone)]
///     pub struct PostPath {
///         pub user_id: i64 => vld::number().int().min(1),
///         pub post_id: i64 => vld::number().int().min(1),
///     }
/// }
///
/// // GET /posts/<user_id>/<post_id>
/// let route = warp::path("posts")
///     .and(vld_path::<PostPath>(&["user_id", "post_id"]))
///     .map(|p: PostPath| {
///         warp::reply::json(&serde_json::json!({
///             "user_id": p.user_id,
///             "post_id": p.post_id
///         }))
///     });
/// ```
pub fn vld_path<T: VldParse + Send + 'static>(
    param_names: &'static [&'static str],
) -> impl Filter<Extract = (T,), Error = Rejection> + Clone {
    warp::path::tail().and_then(move |tail: warp::path::Tail| async move {
        let segments: Vec<&str> = tail.as_str().split('/').filter(|s| !s.is_empty()).collect();

        if segments.len() != param_names.len() {
            return Err(warp::reject::not_found());
        }

        let mut map = serde_json::Map::new();
        for (name, raw) in param_names.iter().zip(segments.iter()) {
            map.insert(name.to_string(), coerce_value(raw));
        }
        let value = serde_json::Value::Object(map);
        T::vld_parse_value(&value).map_err(|e| warp::reject::custom(ValidationFailed { error: e }))
    })
}

// ---------------------------------------------------------------------------
// validate_path_params — standalone validator for pre-extracted params
// ---------------------------------------------------------------------------

/// Validate pre-extracted path parameter pairs against a vld schema.
///
/// Useful for complex routes where static and dynamic segments are
/// interleaved and you extract `String` params with
/// `warp::path::param::<String>()`, then validate them all at once.
///
/// # Example
///
/// ```rust,ignore
/// use vld_warp::validate_path_params;
///
/// vld::schema! {
///     #[derive(Debug, Clone)]
///     pub struct CommentPath {
///         pub user_id: i64 => vld::number().int().min(1),
///         pub post_id: i64 => vld::number().int().min(1),
///         pub comment_id: i64 => vld::number().int().min(1),
///     }
/// }
///
/// // GET /users/<id>/posts/<pid>/comments/<cid>
/// let route = warp::path("users")
///     .and(warp::path::param::<String>())
///     .and(warp::path("posts"))
///     .and(warp::path::param::<String>())
///     .and(warp::path("comments"))
///     .and(warp::path::param::<String>())
///     .and(warp::path::end())
///     .and_then(|uid: String, pid: String, cid: String| async move {
///         validate_path_params::<CommentPath>(&[
///             ("user_id", &uid),
///             ("post_id", &pid),
///             ("comment_id", &cid),
///         ])
///     });
/// ```
pub fn validate_path_params<T: VldParse>(params: &[(&str, &str)]) -> Result<T, Rejection> {
    let mut map = serde_json::Map::new();
    for (name, raw) in params {
        map.insert(name.to_string(), coerce_value(raw));
    }
    let value = serde_json::Value::Object(map);
    T::vld_parse_value(&value).map_err(|e| warp::reject::custom(ValidationFailed { error: e }))
}

// ---------------------------------------------------------------------------
// vld_headers filter
// ---------------------------------------------------------------------------

/// Warp filter that extracts and validates HTTP headers.
///
/// Header names are normalised to snake_case: `Content-Type` → `content_type`.
/// Values are coerced: `"42"` → number, `"true"` → bool, etc.
pub fn vld_headers<T: VldParse + Send + 'static>(
) -> impl Filter<Extract = (T,), Error = Rejection> + Clone {
    warp::header::headers_cloned().and_then(|headers: warp::http::HeaderMap| async move {
        let mut map = serde_json::Map::new();
        for (name, value) in headers.iter() {
            let key = name.as_str().to_lowercase().replace('-', "_");
            if let Ok(v) = value.to_str() {
                map.insert(key, coerce_value(v));
            }
        }
        let value = serde_json::Value::Object(map);

        T::vld_parse_value(&value).map_err(|e| warp::reject::custom(ValidationFailed { error: e }))
    })
}

// ---------------------------------------------------------------------------
// vld_cookie filter
// ---------------------------------------------------------------------------

/// Warp filter that extracts and validates cookies from the `Cookie` header.
///
/// Cookie names are used as-is for schema field matching.
/// Values are coerced: `"42"` → number, `"true"` → bool, etc.
pub fn vld_cookie<T: VldParse + Send + 'static>(
) -> impl Filter<Extract = (T,), Error = Rejection> + Clone {
    warp::header::optional::<String>("cookie").and_then(
        |cookie_header: Option<String>| async move {
            let value = cookies_to_json(cookie_header.as_deref().unwrap_or(""));

            T::vld_parse_value(&value)
                .map_err(|e| warp::reject::custom(ValidationFailed { error: e }))
        },
    )
}

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{
        handle_rejection, validate_path_params, vld_cookie, vld_form, vld_headers, vld_json,
        vld_param, vld_path, vld_query, InvalidJson, ValidationFailed,
    };
    pub use vld::prelude::*;
}
