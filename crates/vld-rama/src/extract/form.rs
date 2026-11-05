use std::future::Future;

use crate::rejection::VldRejection;
use rama::http::body::util::BodyExt;
use rama::http::service::web::extract::{FromRequest, FromRequestBody};
use rama::http::{Body, Method, Request};
use vld::schema::VldParse;
use vld_http_common::{parse_form_body, query_string_to_json, DEFAULT_BODY_LIMIT};

/// Rama extractor that validates **URL-encoded form bodies**.
///
/// For `GET`, the query string is used. For other methods, the body is parsed
/// as `application/x-www-form-urlencoded` (413 beyond [`DEFAULT_BODY_LIMIT`]).
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
        let query_string = (parts.method == Method::GET).then(|| {
            parts
                .uri
                .query()
                .map(|q| q.as_encoded_str().into_owned())
                .unwrap_or_default()
        });

        async move {
            let value = match query_string {
                Some(qs) => query_string_to_json(&qs),
                None => {
                    let bytes = body
                        .limited(DEFAULT_BODY_LIMIT)
                        .collect()
                        .await
                        .map_err(|e| VldRejection::from_body_error(&e))?
                        .to_bytes();
                    parse_form_body(&bytes)?
                }
            };

            let parsed =
                vld_http_common::parse_coerced::<T>(&value).map_err(VldRejection::from_vld)?;
            Ok(VldForm(parsed))
        }
    }
}
