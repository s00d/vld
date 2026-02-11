//! # vld-tower — Tower middleware for `vld` validation
//!
//! A universal [`tower::Layer`] that validates incoming HTTP JSON request
//! bodies against a `vld` schema. Works with **any** Tower-compatible
//! framework: Axum, Hyper, Tonic, Warp, etc.
//!
//! On **success** the validated struct is stored in
//! [`http::Request::extensions`] so downstream handlers can retrieve it
//! without re-parsing. The original body bytes are forwarded as-is.
//!
//! On **failure** a `422 Unprocessable Entity` JSON response is returned
//! immediately — the inner service is never called.
//!
//! # Quick Start (with Axum)
//!
//! ```rust,no_run
//! use vld::prelude::*;
//! use vld_tower::ValidateJsonLayer;
//!
//! vld::schema! {
//!     #[derive(Debug, Clone)]
//!     pub struct CreateUser {
//!         pub name: String  => vld::string().min(2).max(100),
//!         pub email: String => vld::string().email(),
//!     }
//! }
//!
//! // Apply as a layer — works with any Tower-based router
//! // let app = Router::new()
//! //     .route("/users", post(handler))
//! //     .layer(ValidateJsonLayer::<CreateUser>::new());
//! ```

use bytes::Bytes;
use http::{Request, Response, StatusCode};
use http_body::Body;
use http_body_util::BodyExt;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};
use vld::schema::VldParse;

// ---------------------------------------------------------------------------
// Layer
// ---------------------------------------------------------------------------

/// A [`tower_layer::Layer`] that validates JSON request bodies with `vld`.
///
/// The type parameter `T` is the validated struct (must implement
/// [`VldParse`] + [`Clone`] + [`Send`] + [`Sync`] + `'static`).
///
/// # Behaviour
///
/// 1. Reads the full request body.
/// 2. Parses as JSON and validates via `T::vld_parse_value()`.
/// 3. **Valid** — inserts `T` into request extensions, re-attaches the
///    body bytes, and calls the inner service.
/// 4. **Invalid** — returns `422 Unprocessable Entity` with a JSON body
///    containing the validation errors. The inner service is **not** called.
///
/// Requests without `Content-Type: application/json` (or missing content
/// type) are **passed through** without validation.
#[derive(Clone)]
pub struct ValidateJsonLayer<T> {
    _marker: PhantomData<fn() -> T>,
}

impl<T> ValidateJsonLayer<T> {
    /// Create a new validation layer.
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<T> Default for ValidateJsonLayer<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, T> tower_layer::Layer<S> for ValidateJsonLayer<T> {
    type Service = ValidateJsonService<S, T>;

    fn layer(&self, inner: S) -> Self::Service {
        ValidateJsonService {
            inner,
            _marker: PhantomData,
        }
    }
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// The middleware [`Service`](tower_service::Service) created by
/// [`ValidateJsonLayer`].
#[derive(Clone)]
pub struct ValidateJsonService<S, T> {
    inner: S,
    _marker: PhantomData<fn() -> T>,
}

impl<S, T, ReqBody, ResBody> tower_service::Service<Request<ReqBody>> for ValidateJsonService<S, T>
where
    S: tower_service::Service<Request<http_body_util::Full<Bytes>>, Response = Response<ResBody>>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    ReqBody: Body + Send + 'static,
    ReqBody::Data: Send,
    ReqBody::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    ResBody: From<http_body_util::Full<Bytes>> + Send + 'static,
    T: VldParse + Clone + Send + Sync + 'static,
{
    type Response = Response<ResBody>;
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let mut inner = self.inner.clone();
        // Swap so `self` is ready for next call (standard Tower pattern)
        std::mem::swap(&mut self.inner, &mut inner);

        Box::pin(async move {
            let is_json = req
                .headers()
                .get(http::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|ct| ct.starts_with("application/json"))
                .unwrap_or(false);

            if !is_json {
                // Pass through non-JSON requests untouched
                let (parts, body) = req.into_parts();
                let bytes = body
                    .collect()
                    .await
                    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?
                    .to_bytes();
                let new_req = Request::from_parts(parts, http_body_util::Full::new(bytes));
                return inner.call(new_req).await.map_err(Into::into);
            }

            // Collect body bytes
            let (parts, body) = req.into_parts();
            let bytes = body
                .collect()
                .await
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?
                .to_bytes();

            // Parse JSON
            let json_value: serde_json::Value = match serde_json::from_slice(&bytes) {
                Ok(v) => v,
                Err(e) => {
                    let error_body = serde_json::json!({
                        "error": "Invalid JSON",
                        "message": e.to_string(),
                    });
                    let resp = Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .header(http::header::CONTENT_TYPE, "application/json")
                        .body(ResBody::from(http_body_util::Full::new(Bytes::from(
                            serde_json::to_vec(&error_body).unwrap_or_default(),
                        ))))
                        .unwrap();
                    return Ok(resp);
                }
            };

            // Validate with vld
            match T::vld_parse_value(&json_value) {
                Ok(validated) => {
                    let mut new_req = Request::from_parts(parts, http_body_util::Full::new(bytes));
                    // Store validated struct in extensions
                    new_req.extensions_mut().insert(validated);
                    inner.call(new_req).await.map_err(Into::into)
                }
                Err(vld_err) => {
                    let issues: Vec<serde_json::Value> = vld_err
                        .issues
                        .iter()
                        .map(|issue| {
                            serde_json::json!({
                                "path": issue.path.iter()
                                    .map(|p| p.to_string())
                                    .collect::<Vec<_>>()
                                    .join("."),
                                "message": issue.message,
                            })
                        })
                        .collect();

                    let error_body = serde_json::json!({
                        "error": "Validation failed",
                        "issues": issues,
                    });

                    let resp = Response::builder()
                        .status(StatusCode::UNPROCESSABLE_ENTITY)
                        .header(http::header::CONTENT_TYPE, "application/json")
                        .body(ResBody::from(http_body_util::Full::new(Bytes::from(
                            serde_json::to_vec(&error_body).unwrap_or_default(),
                        ))))
                        .unwrap();
                    Ok(resp)
                }
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Helper: extract validated value from request extensions
// ---------------------------------------------------------------------------

/// Extract the validated value from request extensions.
///
/// The [`ValidateJsonService`] middleware stores the parsed and validated
/// struct in the request's extensions map. Use this function (or
/// `req.extensions().get::<T>()` directly) to retrieve it.
///
/// # Panics
///
/// Panics if `T` is not present in extensions (i.e. the middleware was
/// not applied).
pub fn validated<T: Clone + Send + Sync + 'static>(req: &Request<impl Body>) -> T {
    req.extensions()
        .get::<T>()
        .expect(
            "vld-tower: validated value not found in request extensions. \
                 Make sure ValidateJsonLayer is applied.",
        )
        .clone()
}

/// Try to extract the validated value from request extensions.
///
/// Returns `None` if the middleware was not applied or the value type
/// doesn't match.
pub fn try_validated<T: Clone + Send + Sync + 'static>(req: &Request<impl Body>) -> Option<T> {
    req.extensions().get::<T>().cloned()
}

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{try_validated, validated, ValidateJsonLayer, ValidateJsonService};
}
