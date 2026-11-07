//! Rama [`Layer`] that validates JSON request bodies.

use std::convert::Infallible;
use std::fmt;
use std::marker::PhantomData;

use rama::extensions::{Extension, ExtensionsRef};
use rama::http::body::util::BodyExt;
use rama::http::header;
use rama::http::{Body, Request, Response};
use rama::{Layer, Service};
use vld::schema::VldParse;
use vld_http_common::{DEFAULT_BODY_LIMIT, is_json_content_type, parse_json_body};

use crate::rejection::VldRejection;

/// Wrapper stored in Rama [`Extensions`](rama::extensions::Extensions) so
/// validated values do not require a manual `Extension` impl on `T`.
#[derive(Clone)]
pub struct Validated<T>(pub T);

impl<T: fmt::Debug> fmt::Debug for Validated<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Validated").field(&self.0).finish()
    }
}

impl<T> std::ops::Deref for Validated<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: fmt::Debug + Send + Sync + 'static> Extension for Validated<T> {}

/// A Rama [`Layer`] that validates JSON request bodies with `vld`.
///
/// On success inserts [`Validated<T>`] into request extensions and forwards
/// the original body bytes. On failure returns `422` (`413` when the body
/// exceeds the limit) without calling the inner service.
///
/// Requests whose `Content-Type` is not JSON (`application/json` or
/// `application/*+json`) are passed through **untouched** — their body is
/// not read, so use [`validated`] (which returns `Option`) in handlers.
#[derive(Clone)]
pub struct ValidateJsonLayer<T> {
    limit: usize,
    _marker: PhantomData<fn() -> T>,
}

impl<T> ValidateJsonLayer<T> {
    /// Create a new validation layer with the default body limit
    /// ([`DEFAULT_BODY_LIMIT`], 2 MiB).
    #[must_use]
    pub fn new() -> Self {
        Self::with_limit(DEFAULT_BODY_LIMIT)
    }

    /// Create a new validation layer with a custom body limit in bytes.
    #[must_use]
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

impl<S, T> Layer<S> for ValidateJsonLayer<T> {
    type Service = ValidateJsonService<S, T>;

    fn layer(&self, inner: S) -> Self::Service {
        ValidateJsonService {
            inner,
            limit: self.limit,
            _marker: PhantomData,
        }
    }
}

/// Service produced by [`ValidateJsonLayer`].
#[derive(Clone)]
pub struct ValidateJsonService<S, T> {
    inner: S,
    limit: usize,
    _marker: PhantomData<fn() -> T>,
}

impl<S, T> Service<Request> for ValidateJsonService<S, T>
where
    S: Service<Request, Output = Response, Error = Infallible>,
    T: VldParse + Clone + fmt::Debug + Send + Sync + 'static,
{
    type Output = Response;
    type Error = Infallible;

    async fn serve(&self, req: Request) -> Result<Self::Output, Self::Error> {
        use rama::http::service::web::response::IntoResponse;

        let is_json = req
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(is_json_content_type);

        if !is_json {
            return self.inner.serve(req).await;
        }

        let (parts, body) = req.into_parts();
        let bytes = match body.limited(self.limit).collect().await {
            Ok(collected) => collected.to_bytes(),
            Err(e) => return Ok(VldRejection::from_body_error(&e).into_response()),
        };

        let result = parse_json_body(&bytes)
            .map_err(VldRejection::from)
            .and_then(|v| T::vld_parse_value(&v).map_err(VldRejection::from_vld));

        match result {
            Ok(validated) => {
                let new_req = Request::from_parts(parts, Body::from(bytes));
                new_req.extensions().insert(Validated(validated));
                self.inner.serve(new_req).await
            }
            Err(rejection) => Ok(rejection.into_response()),
        }
    }
}

/// Extract the validated value from request extensions.
///
/// Returns `None` when the request was not validated — [`ValidateJsonLayer`]
/// was not applied, the type does not match, or the request was not JSON.
#[must_use]
pub fn validated<T: Clone + fmt::Debug + Send + Sync + 'static>(req: &Request) -> Option<T> {
    req.extensions()
        .get_ref::<Validated<T>>()
        .map(|v| v.0.clone())
}
