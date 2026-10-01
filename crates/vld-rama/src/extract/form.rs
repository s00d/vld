use std::future::Future;

use crate::rejection::VldRejection;
use rama::http::body::util::BodyExt;
use rama::http::service::web::extract::{FromRequest, FromRequestBody};
use rama::http::{Body, Method, Request};
use vld::schema::VldParse;
use vld_http_common::{query_string_to_json, DEFAULT_BODY_LIMIT};

/// Rama extractor that validates **URL-encoded form bodies**.
///
/// For `GET`, the query string is used. For other methods, the body is parsed
/// as `application/x-www-form-urlencoded`.
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
        let is_get = parts.method == Method::GET;
        let query_string = if is_get {
            Some(
                parts
                    .uri
                    .query()
                    .map(|q| q.as_encoded_str().into_owned())
                    .unwrap_or_default(),
            )
        } else {
            None
        };

        async move {
            let value = if let Some(qs) = query_string {
                query_string_to_json(&qs)
            } else {
                let bytes = body
                    .limited(DEFAULT_BODY_LIMIT)
                    .collect()
                    .await
                    .map_err(|_| VldRejection::payload_too_large())?
                    .to_bytes();
                let body_str = std::str::from_utf8(&bytes)
                    .map_err(|_| VldRejection::parse("Form body is not valid UTF-8"))?;
                query_string_to_json(body_str)
            };

            let parsed = T::vld_parse_value(&value).map_err(VldRejection::from_vld)?;
            Ok(VldForm(parsed))
        }
    }
}
