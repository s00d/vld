//! Shared rejection type for all `Vld*` extractors.

use rama::http::service::web::response::IntoResponse;
use rama::http::{header, Response, StatusCode};

/// Rejection returned when validation (or body parsing) fails.
///
/// Converts to a JSON response via [`vld_http_common`] helpers:
/// - validation / parse → **422**
/// - payload too large → **413**
#[derive(Debug)]
pub struct VldRejection {
    error: vld::error::VldError,
    status: StatusCode,
}

impl VldRejection {
    /// Create a rejection from a [`vld::error::VldError`].
    #[must_use]
    pub fn from_vld(error: vld::error::VldError) -> Self {
        Self {
            error,
            status: StatusCode::UNPROCESSABLE_ENTITY,
        }
    }

    /// Create a rejection for a parse / transport error message.
    #[must_use]
    pub fn parse(message: impl Into<String>) -> Self {
        Self::from_vld(vld::error::VldError::single(
            vld::error::IssueCode::ParseError,
            message.into(),
        ))
    }

    /// Create a rejection for an oversized request body.
    #[must_use]
    pub fn payload_too_large() -> Self {
        Self {
            error: vld::error::VldError::single(
                vld::error::IssueCode::ParseError,
                "Payload too large",
            ),
            status: StatusCode::PAYLOAD_TOO_LARGE,
        }
    }

    /// Borrow the underlying validation error.
    #[must_use]
    pub fn error(&self) -> &vld::error::VldError {
        &self.error
    }
}

impl IntoResponse for VldRejection {
    fn into_response(self) -> Response {
        let body = if self.status == StatusCode::PAYLOAD_TOO_LARGE {
            vld_http_common::format_payload_too_large()
        } else {
            vld_http_common::format_vld_error(&self.error)
        };
        (
            self.status,
            [(header::CONTENT_TYPE, "application/json")],
            body.to_string(),
        )
            .into_response()
    }
}

impl std::fmt::Display for VldRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Validation failed: {}", self.error)
    }
}

impl std::error::Error for VldRejection {}
