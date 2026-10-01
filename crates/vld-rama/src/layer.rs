//! Rama [`Layer`] that validates JSON request bodies.

use std::convert::Infallible;
use std::fmt;
use std::marker::PhantomData;

use rama::extensions::{Extension, ExtensionsRef};
use rama::http::body::util::BodyExt;
use rama::http::header;
use rama::http::{Body, Request, Response, StatusCode};
use rama::{Layer, Service};
use vld::schema::VldParse;

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
/// the original body bytes. On failure returns `422` without calling the
/// inner service. Non-JSON content types are passed through unchanged.
#[derive(Clone)]
pub struct ValidateJsonLayer<T> {
    _marker: PhantomData<fn() -> T>,
}

impl<T> ValidateJsonLayer<T> {
    /// Create a new validation layer.
    #[must_use]
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

impl<S, T> Layer<S> for ValidateJsonLayer<T> {
    type Service = ValidateJsonService<S, T>;

    fn layer(&self, inner: S) -> Self::Service {
        ValidateJsonService {
            inner,
            _marker: PhantomData,
        }
    }
}

/// Service produced by [`ValidateJsonLayer`].
#[derive(Clone)]
pub struct ValidateJsonService<S, T> {
    inner: S,
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
        let is_json = req
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|ct| {
                let ct = ct.to_ascii_lowercase();
                ct.starts_with("application/json") || ct.contains("+json")
            })
            .unwrap_or(false);

        let (parts, body) = req.into_parts();
        let bytes = match body
            .limited(vld_http_common::DEFAULT_BODY_LIMIT)
            .collect()
            .await
        {
            Ok(collected) => collected.to_bytes(),
            Err(_) => {
                let error_body = vld_http_common::format_payload_too_large();
                return Ok(json_response(StatusCode::PAYLOAD_TOO_LARGE, &error_body));
            }
        };

        if !is_json {
            let new_req = Request::from_parts(parts, Body::from(bytes));
            return self.inner.serve(new_req).await;
        }

        let json_value: serde_json::Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(e) => {
                let error_body = vld_http_common::format_vld_error(&vld::error::VldError::single(
                    vld::error::IssueCode::ParseError,
                    format!("Invalid JSON: {e}"),
                ));
                return Ok(json_response(StatusCode::UNPROCESSABLE_ENTITY, &error_body));
            }
        };

        match T::vld_parse_value(&json_value) {
            Ok(validated) => {
                let new_req = Request::from_parts(parts, Body::from(bytes));
                new_req.extensions().insert(Validated(validated));
                self.inner.serve(new_req).await
            }
            Err(vld_err) => {
                let error_body = vld_http_common::format_vld_error(&vld_err);
                Ok(json_response(StatusCode::UNPROCESSABLE_ENTITY, &error_body))
            }
        }
    }
}

fn json_response(status: StatusCode, body: &serde_json::Value) -> Response {
    use rama::http::service::web::response::IntoResponse;
    (
        status,
        [(header::CONTENT_TYPE, "application/json")],
        serde_json::to_string(body).unwrap_or_else(|_| "{}".into()),
    )
        .into_response()
}

/// Extract the validated value from request extensions.
///
/// # Panics
///
/// Panics if [`ValidateJsonLayer`] was not applied (or the type does not match).
#[must_use]
pub fn validated<T: Clone + fmt::Debug + Send + Sync + 'static>(req: &Request) -> T {
    req.extensions()
        .get_ref::<Validated<T>>()
        .expect(
            "vld-rama: Validated<T> not found in request extensions. \
             Make sure ValidateJsonLayer is applied.",
        )
        .0
        .clone()
}

/// Try to extract the validated value from request extensions.
#[must_use]
pub fn try_validated<T: Clone + fmt::Debug + Send + Sync + 'static>(req: &Request) -> Option<T> {
    req.extensions()
        .get_ref::<Validated<T>>()
        .map(|v| v.0.clone())
}
