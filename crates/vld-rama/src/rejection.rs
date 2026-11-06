//! Shared rejection type for all `Vld*` extractors.

use rama::http::body::util::LengthLimitError;
use rama::http::service::web::response::IntoResponse;
use rama::http::{Response, StatusCode, header};
use vld_http_common::VldHttpError;

/// Rejection returned when validation (or body parsing) fails; wraps
/// [`VldHttpError`].
///
/// Converts to a JSON response via [`vld_http_common`] helpers:
/// - validation / parse → **422**
/// - payload too large → **413**
/// - wrong `Content-Type` on [`VldJson`](crate::VldJson) /
///   [`VldForm`](crate::VldForm) → **415**
#[derive(Debug)]
pub struct VldRejection(VldHttpError);

impl VldRejection {
    /// Create a rejection from a [`vld::error::VldError`].
    #[must_use]
    pub fn from_vld(error: vld::error::VldError) -> Self {
        Self(VldHttpError::validation(error))
    }

    /// Create a rejection for a parse / transport error message.
    #[must_use]
    pub fn parse(message: impl Into<String>) -> Self {
        Self(VldHttpError::parse(message))
    }

    /// Create a rejection for an oversized request body.
    #[must_use]
    pub fn payload_too_large() -> Self {
        Self(VldHttpError::payload_too_large())
    }

    /// Map a body-collection error: only a length-limit error is 413,
    /// anything else is a 422 parse error.
    pub(crate) fn from_body_error(e: &(dyn std::error::Error + 'static)) -> Self {
        let mut cur = Some(e);
        while let Some(err) = cur {
            if err.is::<LengthLimitError>() {
                return Self::payload_too_large();
            }
            cur = err.source();
        }
        Self::parse(format!("Failed to read request body: {e}"))
    }

    /// Borrow the underlying validation error.
    #[must_use]
    pub fn error(&self) -> &vld::error::VldError {
        self.0.error()
    }

    /// HTTP status of this rejection (422, 413 or 415).
    #[must_use]
    pub fn status(&self) -> StatusCode {
        StatusCode::from_u16(self.0.status()).unwrap_or(StatusCode::UNPROCESSABLE_ENTITY)
    }

    /// JSON response body.
    #[must_use]
    pub fn body(&self) -> serde_json::Value {
        self.0.body()
    }
}

impl From<VldHttpError> for VldRejection {
    fn from(e: VldHttpError) -> Self {
        Self(e)
    }
}

impl IntoResponse for VldRejection {
    fn into_response(self) -> Response {
        json_response(self.status(), &self.body())
    }
}

impl std::fmt::Display for VldRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for VldRejection {}

pub(crate) fn json_response(status: StatusCode, body: &serde_json::Value) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "application/json")],
        body.to_string(),
    )
        .into_response()
}
