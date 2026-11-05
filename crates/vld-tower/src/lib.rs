//! # vld-tower — Tower middleware for `vld` validation
//!
//! A universal [`tower::Layer`](tower_layer::Layer) that validates incoming
//! HTTP JSON request bodies against a `vld` schema. Works with **any**
//! Tower-compatible framework: Axum, Hyper, Tonic, Warp, etc.
//!
//! On **success** the validated struct is stored in
//! [`http::Request::extensions`] so downstream handlers can retrieve it
//! without re-parsing. The original body bytes are forwarded as-is.
//!
//! On **failure** a JSON error response is returned immediately — the inner
//! service is never called:
//!
//! | Situation | Status |
//! |---|---|
//! | schema validation failed / malformed JSON | 422 |
//! | body larger than the limit ([`ValidateJsonLayer::with_limit`], default 2 MiB) | 413 |
//!
//! Requests whose `Content-Type` is not JSON (`application/json` or
//! `application/*+json`, case-insensitive) are passed through **untouched**
//! (body not read). Handlers must therefore treat the validated value as
//! optional: [`validated`] returns `Option<T>`.
//!
//! The inner service receives `Request<`[`ValidatedBody<B>`]`>`: the buffered
//! bytes for validated JSON requests, the original body otherwise.
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
use http_body_util::{BodyExt, Either, Full, LengthLimitError, Limited};
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};
use vld::schema::VldParse;
use vld_http_common::{is_json_content_type, parse_json_body, VldHttpError, DEFAULT_BODY_LIMIT};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Body type seen by the inner service: `Left` holds the buffered bytes of
/// a validated JSON request, `Right` the untouched original body of a
/// non-JSON request.
pub type ValidatedBody<B> = Either<Full<Bytes>, B>;

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
/// 1. Non-JSON requests are forwarded untouched (body not read).
/// 2. JSON bodies are read up to the limit (413 beyond it), parsed and
///    validated via `T::vld_parse_value()`.
/// 3. **Valid** — inserts `T` into request extensions, re-attaches the
///    body bytes, and calls the inner service.
/// 4. **Invalid** — returns `422 Unprocessable Entity` with a JSON body
///    containing the validation errors. The inner service is **not** called.
#[derive(Clone)]
pub struct ValidateJsonLayer<T> {
    limit: usize,
    _marker: PhantomData<fn() -> T>,
}

impl<T> ValidateJsonLayer<T> {
    /// Create a new validation layer with the default body limit
    /// ([`DEFAULT_BODY_LIMIT`], 2 MiB).
    pub fn new() -> Self {
        Self::with_limit(DEFAULT_BODY_LIMIT)
    }

    /// Create a new validation layer with a custom body limit in bytes.
    pub fn with_limit(limit: usize) -> Self {
        Self {
            limit,
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
            limit: self.limit,
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
    limit: usize,
    _marker: PhantomData<fn() -> T>,
}

fn error_response<ResBody: From<Full<Bytes>>>(err: VldHttpError) -> Response<ResBody> {
    let mut resp = Response::new(ResBody::from(Full::new(Bytes::from(
        err.body().to_string(),
    ))));
    *resp.status_mut() =
        StatusCode::from_u16(err.status()).unwrap_or(StatusCode::UNPROCESSABLE_ENTITY);
    resp.headers_mut().insert(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static("application/json"),
    );
    resp
}

impl<S, T, ReqBody, ResBody> tower_service::Service<Request<ReqBody>> for ValidateJsonService<S, T>
where
    S: tower_service::Service<Request<ValidatedBody<ReqBody>>, Response = Response<ResBody>>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    S::Error: Into<BoxError>,
    ReqBody: Body<Data = Bytes> + Send + 'static,
    ReqBody::Error: Into<BoxError>,
    ResBody: From<Full<Bytes>> + Send + 'static,
    T: VldParse + Clone + Send + Sync + 'static,
{
    type Response = Response<ResBody>;
    type Error = BoxError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let mut inner = self.inner.clone();
        // Swap so `self` is ready for next call (standard Tower pattern)
        std::mem::swap(&mut self.inner, &mut inner);
        let limit = self.limit;

        Box::pin(async move {
            let is_json = req
                .headers()
                .get(http::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .is_some_and(is_json_content_type);

            let (parts, body) = req.into_parts();

            if !is_json {
                let req = Request::from_parts(parts, Either::Right(body));
                return inner.call(req).await.map_err(Into::into);
            }

            let bytes = match Limited::new(body, limit).collect().await {
                Ok(collected) => collected.to_bytes(),
                Err(e) if e.is::<LengthLimitError>() => {
                    return Ok(error_response(VldHttpError::payload_too_large()));
                }
                Err(e) => return Err(e),
            };

            let validated = match parse_json_body(&bytes)
                .and_then(|v| T::vld_parse_value(&v).map_err(VldHttpError::validation))
            {
                Ok(v) => v,
                Err(e) => return Ok(error_response(e)),
            };

            let mut req = Request::from_parts(parts, Either::Left(Full::new(bytes)));
            req.extensions_mut().insert(validated);
            inner.call(req).await.map_err(Into::into)
        })
    }
}

// ---------------------------------------------------------------------------
// Helper: extract validated value from request extensions
// ---------------------------------------------------------------------------

/// Extract the validated value from request extensions.
///
/// Returns `None` when the request was not validated — the layer was not
/// applied, the type does not match, or the request was not JSON (non-JSON
/// requests are passed through without validation).
pub fn validated<T: Clone + Send + Sync + 'static>(req: &Request<impl Body>) -> Option<T> {
    req.extensions().get::<T>().cloned()
}

/// Try to extract the validated value from request extensions.
#[deprecated(
    since = "0.5.0",
    note = "use `validated`, which now returns `Option<T>`"
)]
pub fn try_validated<T: Clone + Send + Sync + 'static>(req: &Request<impl Body>) -> Option<T> {
    validated(req)
}

/// Prelude — import everything you need.
pub mod prelude {
    #[allow(deprecated)]
    pub use crate::try_validated;
    pub use crate::{validated, ValidateJsonLayer, ValidateJsonService, ValidatedBody};
    pub use vld::prelude::*;
}
