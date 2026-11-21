#![allow(clippy::result_large_err)]
//! # `vld_web::lambda` — AWS Lambda integration for `vld`
//!
//! Validate API Gateway / ALB / Function URL requests of
//! [`lambda_http`](https://docs.rs/lambda_http) 1.x, and raw
//! `lambda_runtime` events, with `vld` schemas (`lambda_http` with its
//! default features: every event source; `lambda_runtime` is used through
//! its `lambda_http::lambda_runtime` re-export).
//!
//! | Function | Source |
//! |---|---|
//! | [`validate_json`] | JSON body (requires a JSON `Content-Type`) |
//! | [`validate_query`] | `queryStringParameters` ([`RequestExt::query_string_parameters`]) |
//! | [`validate_path`] | `pathParameters` ([`RequestExt::path_parameters`]) |
//! | [`error_response`] | the 422 / 415 response for a [`VldHttpError`] |
//! | [`validate_event`] / [`event_error`] | a raw `LambdaEvent<serde_json::Value>` payload; failures are a [`Diagnostic`] |
//!
//! Requests answer with the same bodies and parse parameters by the same
//! rules as every HTTP module ([error bodies](crate::common#error-bodies)).
//!
//! # Example
//!
//! ```
//! use lambda_http::{Body, Error, Request, Response};
//! use vld_web::lambda::{validate_json, validate_path};
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct UserPath {
//!         pub id: i64 => vld::number().int().min(1),
//!     }
//! }
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct UpdateUser {
//!         pub name: String => vld::string().min(2),
//!     }
//! }
//!
//! async fn handler(req: Request) -> Result<Response<Body>, Error> {
//!     let path: UserPath = match validate_path(&req) {
//!         Ok(p) => p,
//!         Err(resp) => return Ok(resp), // 422
//!     };
//!     let body: UpdateUser = match validate_json(&req) {
//!         Ok(b) => b,
//!         Err(resp) => return Ok(resp), // 422 / 415
//!     };
//!     Ok(Response::new(format!("{}: {}", path.id, body.name).into()))
//! }
//! // lambda_http::run(lambda_http::service_fn(handler)).await
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/web/lambda>
use lambda_http::http::{StatusCode, header};
use lambda_http::lambda_runtime::{Context, Diagnostic, LambdaEvent};
use lambda_http::{Body, Request, RequestExt, Response};
use vld::error::VldError;
use vld::schema::VldParse;

use crate::common::{
    VldHttpError, parse_coerced, parse_json_body, path_params_to_json, query_string_to_json,
    require_json_content_type,
};

/// Validate the JSON body. A missing or non-JSON `Content-Type` is **415**,
/// malformed JSON and schema errors are **422**.
pub fn validate_json<T: VldParse>(request: &Request) -> Result<T, Response<Body>> {
    let content_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());
    require_json_content_type(content_type)
        .and_then(|()| parse_json_body(request.body()))
        .and_then(|value| T::vld_parse_value(&value).map_err(VldHttpError::from))
        .map_err(|e| error_response(&e))
}

/// Validate the query parameters the event carries
/// ([`RequestExt::query_string_parameters`]). Repeated keys become arrays
/// and values are coerced as for every HTTP module's query extractor.
pub fn validate_query<T: VldParse>(request: &Request) -> Result<T, Response<Body>> {
    let query: Vec<String> = request
        .query_string_parameters()
        .iter()
        .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
        .collect();
    validate_coerced(&query_string_to_json(&query.join("&")))
}

/// Validate the path parameters API Gateway matched
/// ([`RequestExt::path_parameters`], already decoded); values are coerced
/// as for every HTTP module's path extractor.
pub fn validate_path<T: VldParse>(request: &Request) -> Result<T, Response<Body>> {
    validate_coerced(&path_params_to_json(request.path_parameters().iter()))
}

/// The JSON error response for `error`: its status (422 / 415 / 413) and
/// body. For a plain [`VldError`] use `error_response(&err.into())` (422).
pub fn error_response(error: &VldHttpError) -> Response<Body> {
    let mut response = Response::new(Body::from(error.body().to_string()));
    *response.status_mut() =
        StatusCode::from_u16(error.status()).unwrap_or(StatusCode::UNPROCESSABLE_ENTITY);
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/json"),
    );
    response
}

/// Validate the payload of a raw `lambda_runtime` event (SQS, EventBridge,
/// a direct invocation, …) and hand back the payload and its context.
///
/// ```ignore
/// async fn handler(event: LambdaEvent<serde_json::Value>) -> Result<String, Diagnostic> {
///     let (order, _ctx) = vld_web::lambda::validate_event::<Order>(event)?;
///     Ok(order.id)
/// }
/// ```
pub fn validate_event<T: VldParse>(
    event: LambdaEvent<serde_json::Value>,
) -> Result<(T, Context), Diagnostic> {
    let LambdaEvent { payload, context } = event;
    T::vld_parse_value(&payload)
        .map(|value| (value, context))
        .map_err(|e| event_error(&e))
}

/// A [`Diagnostic`] for a failed invocation: `errorType`
/// `"ValidationError"`, `errorMessage` the HTTP 422 body as JSON text.
pub fn event_error(error: &VldError) -> Diagnostic {
    Diagnostic {
        error_type: "ValidationError".into(),
        error_message: crate::common::format_vld_error(error).to_string(),
    }
}

fn validate_coerced<T: VldParse>(value: &serde_json::Value) -> Result<T, Response<Body>> {
    parse_coerced(value).map_err(|e| error_response(&e.into()))
}

/// Percent-encode everything but unreserved characters, so the decoded
/// parameters survive the round trip through [`query_string_to_json`].
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Prelude — import everything you need.
pub mod prelude {
    pub use super::{
        error_response, event_error, validate_event, validate_json, validate_path, validate_query,
    };
    pub use vld::prelude::*;
}
