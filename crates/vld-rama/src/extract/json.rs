use std::future::Future;

use crate::rejection::VldRejection;
use rama::http::body::util::BodyExt;
use rama::http::service::web::extract::{FromRequest, FromRequestBody};
use rama::http::{Body, Request};
use vld::schema::VldParse;
use vld_http_common::DEFAULT_BODY_LIMIT;

/// Rama extractor that validates **JSON request bodies**.
///
/// Drop-in alternative to `rama::http::service::web::extract::Json<T>` that
/// uses `vld` instead of `serde::Deserialize`.
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
        _parts: &rama::http::request::Parts,
        body: Body,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send + 'static {
        async move {
            let bytes = body
                .limited(DEFAULT_BODY_LIMIT)
                .collect()
                .await
                .map_err(|_| VldRejection::payload_too_large())?
                .to_bytes();

            let value: serde_json::Value = serde_json::from_slice(&bytes)
                .map_err(|e| VldRejection::parse(format!("Invalid JSON: {e}")))?;

            let parsed = T::vld_parse_value(&value).map_err(VldRejection::from_vld)?;
            Ok(VldJson(parsed))
        }
    }
}
