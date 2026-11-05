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
//! | [`vld_param::<T>(name)`](vld_param) | Single path segment |
//! | [`vld_path::<T>(names)`](vld_path) | All remaining path segments (tail) |
//! | [`validate_path_params::<T>(pairs)`](validate_path_params) | Pre-extracted path params |
//! | [`vld_headers::<T>()`] | HTTP headers |
//! | [`vld_cookie::<T>()`] | Cookie values |
//!
//! Every filter rejects with [`VldRejection`] (wrapping [`VldHttpError`]).
//! Use [`handle_rejection`] with `.recover()` to turn rejections into JSON
//! (see the [error body format](vld_http_common#error-bodies)):
//! validation / malformed body → 422, body over the limit → 413, `vld_json`
//! without a JSON `Content-Type` (`application/json` or `application/*+json`)
//! or `vld_form` without `application/x-www-form-urlencoded` → 415, plus
//! the usual warp rejections (404, 405, 400, 411, 415) and 500 for anything
//! unknown.
//!
//! # Body limit
//!
//! `vld_json` / `vld_form` read at most
//! [`DEFAULT_BODY_LIMIT`](vld_http_common::DEFAULT_BODY_LIMIT) (2 MiB);
//! [`vld_json_limit`] / [`vld_form_limit`] take an explicit limit.
//!
//! Works with warp 0.3 and 0.4 (the dependency is `>=0.3, <0.5`; there are
//! no version features).

use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures_util::{Stream, StreamExt};
use std::convert::Infallible;
use vld::schema::VldParse;
use vld_http_common::{
    cookie_headers_to_json, headers_to_json, parse_coerced, parse_form_body, parse_json_body,
    path_params_to_json, percent_decode, query_string_to_json, require_form_content_type,
    require_json_content_type, VldHttpError,
};
use warp::http::{header, HeaderValue, StatusCode};
use warp::reject::Reject;
use warp::{Filter, Rejection, Reply};

// ---------------------------------------------------------------------------
// Rejection type
// ---------------------------------------------------------------------------

/// The rejection of every `vld_*` filter: validation / parse failure (422),
/// body too large (413) or wrong `Content-Type` (415). [`handle_rejection`]
/// answers it with [`status`](Self::status) and [`body`](Self::body).
#[derive(Debug)]
pub struct VldRejection(VldHttpError);

impl VldRejection {
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

impl From<VldHttpError> for VldRejection {
    fn from(e: VldHttpError) -> Self {
        Self(e)
    }
}

impl Reject for VldRejection {}

fn reject(e: impl Into<VldHttpError>) -> Rejection {
    warp::reject::custom(VldRejection(e.into()))
}

/// Validate a value coerced from text (query, path, form, headers, cookies).
fn validate<T: VldParse>(value: &serde_json::Value) -> Result<T, Rejection> {
    parse_coerced(value).map_err(reject)
}

/// The raw value of header `name`, if any. Unlike
/// `warp::header::optional::<String>`, a non-ASCII value is not a 400.
fn raw_header(
    name: &'static str,
) -> impl Filter<Extract = (Option<HeaderValue>,), Error = Infallible> + Clone {
    warp::header::value(name)
        .map(Some)
        .or(warp::any().map(|| None))
        .unify()
}

fn header_str(value: &Option<HeaderValue>) -> Option<&str> {
    value.as_ref().and_then(|v| v.to_str().ok())
}

/// Read the whole body, streaming, up to `limit` bytes. Works without a
/// `Content-Length` header (chunked uploads).
fn limited_body(limit: u64) -> impl Filter<Extract = (Bytes,), Error = Rejection> + Clone {
    warp::body::stream().and_then(move |stream| collect_limited(stream, limit))
}

async fn collect_limited<S, B>(stream: S, limit: u64) -> Result<Bytes, Rejection>
where
    S: Stream<Item = Result<B, warp::Error>>,
    B: Buf,
{
    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    let mut stream = std::pin::pin!(stream);
    let mut buf = BytesMut::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| {
            reject(VldHttpError::parse(format!(
                "Failed to read request body: {e}"
            )))
        })?;
        if buf.len().saturating_add(chunk.remaining()) > limit {
            return Err(reject(VldHttpError::payload_too_large()));
        }
        buf.put(chunk);
    }
    Ok(buf.freeze())
}

// ---------------------------------------------------------------------------
// vld_json filter
// ---------------------------------------------------------------------------

/// Warp filter that extracts and validates a JSON body.
///
/// Returns the validated `T` or rejects with [`VldRejection`] (422; 413 for
/// a body over [`DEFAULT_BODY_LIMIT`](vld_http_common::DEFAULT_BODY_LIMIT);
/// 415 without a JSON `Content-Type`).
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
    vld_json_limit(vld_http_common::DEFAULT_BODY_LIMIT as u64)
}

/// Like [`vld_json`], but with an explicit body size limit (bytes).
pub fn vld_json_limit<T: VldParse + Send + 'static>(
    limit: u64,
) -> impl Filter<Extract = (T,), Error = Rejection> + Clone {
    raw_header("content-type")
        .and_then(|ct: Option<HeaderValue>| async move {
            require_json_content_type(header_str(&ct)).map_err(reject)
        })
        .untuple_one()
        .and(limited_body(limit))
        .and_then(|bytes: Bytes| async move {
            let value = parse_json_body(&bytes).map_err(reject)?;
            T::vld_parse_value(&value).map_err(reject)
        })
}

// ---------------------------------------------------------------------------
// vld_query filter
// ---------------------------------------------------------------------------

/// Warp filter that extracts and validates query parameters.
///
/// See [`vld_http_common::parse_query_string`] for coercion and array rules.
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
        .and_then(|qs: String| async move { validate(&query_string_to_json(&qs)) })
}

// ---------------------------------------------------------------------------
// Recovery handler
// ---------------------------------------------------------------------------

/// Recovery handler that converts rejections into JSON responses.
///
/// | Rejection | Status |
/// |---|---|
/// | [`VldRejection`] | its [`status`](VldRejection::status) (422 / 413 / 415) and [`body`](VldRejection::body) |
/// | `PayloadTooLarge` | 413 |
/// | `MethodNotAllowed` | 405 |
/// | `UnsupportedMediaType` | 415 |
/// | `LengthRequired` | 411 |
/// | `MissingHeader`, `InvalidHeader`, `MissingCookie`, `InvalidQuery` | 400 |
/// | not found | 404 |
/// | anything else | 500 |
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
    use warp::reject as r;

    let (status, body) = if let Some(e) = err.find::<VldRejection>() {
        (e.status(), e.body())
    } else if err.find::<r::PayloadTooLarge>().is_some() {
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            vld_http_common::format_payload_too_large(),
        )
    } else if err.is_not_found() {
        (
            StatusCode::NOT_FOUND,
            vld_http_common::format_generic_error("Not Found"),
        )
    } else {
        let known = |status: StatusCode, msg: String| (status, msg);
        // Same priority as warp: any specific error beats 405, 405 beats 404.
        let (status, msg) = if let Some(e) = err.find::<r::UnsupportedMediaType>() {
            known(StatusCode::UNSUPPORTED_MEDIA_TYPE, e.to_string())
        } else if let Some(e) = err.find::<r::LengthRequired>() {
            known(StatusCode::LENGTH_REQUIRED, e.to_string())
        } else if let Some(e) = err.find::<r::MissingHeader>() {
            known(StatusCode::BAD_REQUEST, e.to_string())
        } else if let Some(e) = err.find::<r::InvalidHeader>() {
            known(StatusCode::BAD_REQUEST, e.to_string())
        } else if let Some(e) = err.find::<r::MissingCookie>() {
            known(StatusCode::BAD_REQUEST, e.to_string())
        } else if let Some(e) = err.find::<r::InvalidQuery>() {
            known(StatusCode::BAD_REQUEST, e.to_string())
        } else if let Some(e) = err.find::<r::MethodNotAllowed>() {
            known(StatusCode::METHOD_NOT_ALLOWED, e.to_string())
        } else {
            known(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error".to_string(),
            )
        };
        (status, vld_http_common::format_generic_error(&msg))
    };

    Ok(warp::reply::with_status(warp::reply::json(&body), status))
}

// ---------------------------------------------------------------------------
// vld_form filter
// ---------------------------------------------------------------------------

/// Warp filter that extracts and validates a URL-encoded form body
/// (`application/x-www-form-urlencoded`, 415 otherwise).
///
/// Values are parsed the same way as query parameters.
pub fn vld_form<T: VldParse + Send + 'static>(
) -> impl Filter<Extract = (T,), Error = Rejection> + Clone {
    vld_form_limit(vld_http_common::DEFAULT_BODY_LIMIT as u64)
}

/// Like [`vld_form`], but with an explicit body size limit (bytes).
pub fn vld_form_limit<T: VldParse + Send + 'static>(
    limit: u64,
) -> impl Filter<Extract = (T,), Error = Rejection> + Clone {
    raw_header("content-type")
        .and_then(|ct: Option<HeaderValue>| async move {
            require_form_content_type(header_str(&ct)).map_err(reject)
        })
        .untuple_one()
        .and(limited_body(limit))
        .and_then(|bytes: Bytes| async move { validate(&parse_form_body(&bytes).map_err(reject)?) })
}

// ---------------------------------------------------------------------------
// vld_param — single path parameter filter
// ---------------------------------------------------------------------------

/// Warp filter that extracts and validates **a single path segment**.
///
/// Works like `warp::path::param::<String>()` but percent-decodes and
/// coerces the raw value and validates via `T::vld_parse_value()`.
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
        validate(&path_params_to_json([(
            name,
            percent_decode(&raw).as_str(),
        )]))
    })
}

// ---------------------------------------------------------------------------
// vld_path — multi-param tail filter
// ---------------------------------------------------------------------------

/// Warp filter that extracts and validates **all remaining path segments**.
///
/// Uses `warp::path::tail()` internally — all segments after the current
/// position are consumed, percent-decoded and mapped to `param_names` in
/// order. The number of remaining segments **must equal** the number of
/// names; otherwise the request is rejected with 404 (not found).
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
        let segments: Vec<String> = tail
            .as_str()
            .split('/')
            .filter(|s| !s.is_empty())
            .map(percent_decode)
            .collect();

        if segments.len() != param_names.len() {
            return Err(warp::reject::not_found());
        }

        validate(&path_params_to_json(
            param_names
                .iter()
                .copied()
                .zip(segments.iter().map(String::as_str)),
        ))
    })
}

// ---------------------------------------------------------------------------
// validate_path_params — standalone validator for pre-extracted params
// ---------------------------------------------------------------------------

/// Validate pre-extracted path parameter pairs against a vld schema.
///
/// Values are percent-decoded (warp's `path::param::<String>()` yields the
/// raw, still-encoded segment) and coerced.
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
    let decoded: Vec<(&str, String)> = params
        .iter()
        .map(|(k, v)| (*k, percent_decode(v)))
        .collect();
    validate(&path_params_to_json(
        decoded.iter().map(|(k, v)| (*k, v.as_str())),
    ))
}

// ---------------------------------------------------------------------------
// vld_headers filter
// ---------------------------------------------------------------------------

/// Warp filter that extracts and validates HTTP headers.
///
/// See [`vld_http_common::headers_to_json`]: names become snake_case
/// (`Content-Type` → `content_type`), names containing `_` are ignored,
/// repeated headers are joined with `", "`.
pub fn vld_headers<T: VldParse + Send + 'static>(
) -> impl Filter<Extract = (T,), Error = Rejection> + Clone {
    warp::header::headers_cloned().and_then(|headers: warp::http::HeaderMap| async move {
        validate(&headers_to_json(
            headers.iter().map(|(n, v)| (n.as_str(), v.as_bytes())),
        ))
    })
}

// ---------------------------------------------------------------------------
// vld_cookie filter
// ---------------------------------------------------------------------------

/// Warp filter that extracts and validates cookies from the `Cookie`
/// headers (all of them; the first occurrence of a name wins; a non-ASCII
/// value is not a 400).
///
/// Cookie names are used as-is for schema field matching.
/// Values are unquoted, percent-decoded and coerced.
pub fn vld_cookie<T: VldParse + Send + 'static>(
) -> impl Filter<Extract = (T,), Error = Rejection> + Clone {
    warp::header::headers_cloned().and_then(|headers: warp::http::HeaderMap| async move {
        validate(&cookie_headers_to_json(
            headers.get_all(header::COOKIE).iter().map(|v| v.as_bytes()),
        ))
    })
}

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{
        handle_rejection, validate_path_params, vld_cookie, vld_form, vld_form_limit, vld_headers,
        vld_json, vld_json_limit, vld_param, vld_path, vld_query, VldRejection,
    };
    pub use vld::prelude::*;
}
