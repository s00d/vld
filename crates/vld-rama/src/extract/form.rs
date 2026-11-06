use std::future::Future;

use crate::rejection::VldRejection;
use rama::http::service::web::extract::{FromRequest, FromRequestBody};
use rama::http::{Body, Request, header};
use vld::schema::VldParse;
use vld_http_common::{parse_form_body, require_form_content_type};

/// Rama extractor that validates **URL-encoded form bodies**
/// (`application/x-www-form-urlencoded`, 415 otherwise).
///
/// Only the body is read — never the query string, whatever the method
/// (use [`VldQuery`](crate::VldQuery) for that). Bodies above the
/// [`VldBodyLimit`](crate::VldBodyLimit) extension (default 2 MiB) are 413.
pub struct VldForm<T>(pub T);

impl<T> FromRequest for VldForm<T>
where
    T: VldParse + Send + Sync + 'static,
{
    type Rejection = VldRejection;

    async fn from_request(req: Request) -> Result<Self, Self::Rejection> {
        let (parts, body) = req.into_parts();
        Self::from_request_body(&parts, body).await
    }
}

impl<T> FromRequestBody for VldForm<T>
where
    T: VldParse + Send + Sync + 'static,
{
    // Copy needed parts first — future is `'static` (Rama FromRequestBody contract).
    #[allow(clippy::manual_async_fn)]
    fn from_request_body(
        parts: &rama::http::request::Parts,
        body: Body,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send + 'static {
        let content_type = require_form_content_type(
            parts
                .headers
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
        );
        let limit = super::body_limit(parts);
        async move {
            content_type?;
            let bytes = super::read_body(body, limit).await?;
            let value = parse_form_body(&bytes)?;
            let parsed =
                vld_http_common::parse_coerced::<T>(&value).map_err(VldRejection::from_vld)?;
            Ok(VldForm(parsed))
        }
    }
}
