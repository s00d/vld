//! # vld-tower — Tower middleware for `vld`
//!
//! A [`tower::Layer`](tower_layer::Layer) that validates JSON request bodies
//! against a `vld` schema, for any Tower service over the [`http`] types: an
//! Axum `Router` (see `examples/tower_axum.rs`), a hyper service via
//! `hyper-util`'s `TowerToHyperService`, or a plain `service_fn`. It depends
//! only on `tower-layer` / `tower-service` / `http`, not on a web framework.
//!
//! On **success** the validated value is stored in the request extensions as
//! [`Validated<T>`] and the body bytes are forwarded. On **failure** a JSON
//! error response is returned and the inner service is not called (see the
//! [error body format](vld_http_common#error-bodies)):
//!
//! | Situation | Status |
//! |---|---|
//! | schema validation failed / malformed JSON / body read error | 422 |
//! | body larger than the limit ([`ValidateJsonLayer::with_limit`], default 2 MiB) | 413 |
//!
//! Requests whose `Content-Type` is not JSON (`application/json` or
//! `application/*+json`, case-insensitive) are passed through **untouched**
//! (body not read), so [`validated`] returns `Option<T>`.
//!
//! The inner service receives `Request<`[`ValidatedBody<B>`]`>`: the buffered
//! bytes for validated JSON requests, the original body otherwise. Responses
//! are `Response<ValidatedBody<ResBody>>`: the error body or the inner
//! service's. The layer never fails itself; its error type is the inner
//! service's.
//!
//! # Example
//!
//! ```
//! use bytes::Bytes;
//! use http::{Request, Response, StatusCode};
//! use http_body_util::Full;
//! use tower::{ServiceBuilder, ServiceExt};
//! use vld_tower::{validated, ValidateJsonLayer, ValidatedBody};
//!
//! vld::schema! {
//!     #[derive(Debug, Clone)]
//!     pub struct CreateUser {
//!         pub name: String  => vld::string().min(2),
//!         pub email: String => vld::string().email(),
//!     }
//! }
//!
//! async fn handler(
//!     req: Request<ValidatedBody<Full<Bytes>>>,
//! ) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
//!     let name = validated::<CreateUser>(&req).map_or("not JSON".into(), |u| u.name);
//!     Ok(Response::new(Full::new(Bytes::from(name))))
//! }
//!
//! # #[tokio::main]
//! # async fn main() {
//! let svc = ServiceBuilder::new()
//!     .layer(ValidateJsonLayer::<CreateUser>::new())
//!     .service_fn(handler);
//!
//! let req = Request::post("/users")
//!     .header("content-type", "application/json")
//!     .body(Full::new(Bytes::from(r#"{"name":"A","email":"bad"}"#)))
//!     .unwrap();
//! let resp = svc.oneshot(req).await.unwrap();
//! assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
//! # }
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/web/tower>
#![warn(missing_docs)]

use bytes::Bytes;
use http::{Request, Response, StatusCode};
use http_body::Body;
use http_body_util::{BodyExt, Either, Full, LengthLimitError, Limited};
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};
use vld::schema::VldParse;
use vld_http_common::{DEFAULT_BODY_LIMIT, VldHttpError, is_json_content_type, parse_json_body};

/// Body type seen by the inner service (`Left`: the buffered bytes of a
/// validated JSON request, `Right`: the untouched original body of a
/// non-JSON request) and returned by the layer (`Left`: a JSON error body,
/// `Right`: the inner service's body).
pub type ValidatedBody<B> = Either<Full<Bytes>, B>;

/// The validated value, stored in the request extensions by
/// [`ValidateJsonLayer`] (a wrapper, so it never collides with other
/// extensions of type `T`).
#[derive(Debug, Clone)]
pub struct Validated<T>(pub T);

impl<T> std::ops::Deref for Validated<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

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
/// 2. JSON bodies are read up to the limit (413 beyond it, 422 on a read
///    error), parsed and validated via `T::vld_parse_value()`.
/// 3. **Valid** — inserts [`Validated<T>`] into request extensions,
///    re-attaches the body bytes, and calls the inner service.
/// 4. **Invalid** — returns `422 Unprocessable Entity` (`413` when the body
///    is over the limit) with a JSON error body. The inner service is **not**
///    called.
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

fn error_response<B>(err: VldHttpError) -> Response<ValidatedBody<B>> {
    let mut resp = Response::new(Either::Left(Full::new(Bytes::from(err.body().to_string()))));
    *resp.status_mut() =
        StatusCode::from_u16(err.status()).unwrap_or(StatusCode::UNPROCESSABLE_ENTITY);
    resp.headers_mut().insert(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static("application/json"),
    );
    resp
}

/// 413 when the length limit was hit (anywhere in the source chain), else a
/// 422 parse error.
fn body_error(e: &(dyn std::error::Error + 'static)) -> VldHttpError {
    if std::iter::successors(Some(e), |e| e.source()).any(|e| e.is::<LengthLimitError>()) {
        VldHttpError::payload_too_large()
    } else {
        VldHttpError::parse(format!("Failed to read request body: {e}"))
    }
}

impl<S, T, ReqBody, ResBody> tower_service::Service<Request<ReqBody>> for ValidateJsonService<S, T>
where
    S: tower_service::Service<Request<ValidatedBody<ReqBody>>, Response = Response<ResBody>>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
    ReqBody: Body<Data = Bytes> + Send + 'static,
    ReqBody::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    ResBody: Send + 'static,
    T: VldParse + Clone + Send + Sync + 'static,
{
    type Response = Response<ValidatedBody<ResBody>>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
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
                return Ok(inner.call(req).await?.map(Either::Right));
            }

            let bytes = match Limited::new(body, limit).collect().await {
                Ok(collected) => collected.to_bytes(),
                Err(e) => return Ok(error_response(body_error(&*e))),
            };

            let validated = match parse_json_body(&bytes)
                .and_then(|v| T::vld_parse_value(&v).map_err(VldHttpError::validation))
            {
                Ok(v) => v,
                Err(e) => return Ok(error_response(e)),
            };

            let mut req = Request::from_parts(parts, Either::Left(Full::new(bytes)));
            req.extensions_mut().insert(Validated(validated));
            Ok(inner.call(req).await?.map(Either::Right))
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
    req.extensions().get::<Validated<T>>().map(|v| v.0.clone())
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
    pub use crate::{ValidateJsonLayer, ValidateJsonService, Validated, ValidatedBody, validated};
    pub use vld::prelude::*;
}
