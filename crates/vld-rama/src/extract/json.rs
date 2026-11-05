use std::future::Future;

use crate::rejection::VldRejection;
use rama::http::body::util::BodyExt;
use rama::http::service::web::extract::{FromRequest, FromRequestBody};
use rama::http::{header, Body, Request};
use vld::schema::VldParse;
use vld_http_common::{parse_json_body, require_json_content_type, DEFAULT_BODY_LIMIT};

/// Rama extractor that validates **JSON request bodies**.
///
/// Drop-in alternative to `rama::http::service::web::extract::Json<T>` that
/// uses `vld` instead of `serde::Deserialize`. Bodies larger than
/// [`DEFAULT_BODY_LIMIT`] (2 MiB) are rejected with 413; a missing or
/// non-JSON `Content-Type` (see [`vld_http_common::is_json_content_type`])
/// with 415.
pub struct VldJson<T>(pub T);

impl<T> FromRequest for VldJson<T>
where
    T: VldParse + Send + Sync + 'static,
{
    type Rejection = VldRejection;

    async fn from_request(req: Request) -> Result<Self, Self::Rejection> {
        let (parts, body) = req.into_parts();
        Self::from_request_body(&parts, body).await
    }
}

impl<T> FromRequestBody for VldJson<T>
where
    T: VldParse + Send + Sync + 'static,
{
    // Must not capture `&Parts` — future is `'static` (Rama FromRequestBody contract).
    #[allow(clippy::manual_async_fn)]
    fn from_request_body(
        parts: &rama::http::request::Parts,
        body: Body,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send + 'static {
        let content_type = require_json_content_type(
            parts
                .headers
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
        );
        async move {
            content_type?;
            let bytes = body
                .limited(DEFAULT_BODY_LIMIT)
                .collect()
                .await
                .map_err(|e| VldRejection::from_body_error(&e))?
                .to_bytes();

            let value = parse_json_body(&bytes)?;
            let parsed = T::vld_parse_value(&value).map_err(VldRejection::from_vld)?;
            Ok(VldJson(parsed))
        }
    }
}
