use crate::common::headers_to_json;
use crate::rama::rejection::VldRejection;
use rama::http::request::Parts;
use rama::http::service::web::extract::FromPartsStateRefPair;
use vld::schema::VldParse;

/// Rama extractor that validates **HTTP headers**.
///
/// See [`crate::common::headers_to_json`]: names become snake_case
/// (`X-Request-Id` → `x_request_id`), names containing `_` are ignored,
/// repeated headers are joined with `", "`.
pub struct VldHeaders<T>(pub T);

impl<T, State> FromPartsStateRefPair<State> for VldHeaders<T>
where
    T: VldParse + Send + Sync + 'static,
    State: Send + Sync + 'static,
{
    type Rejection = VldRejection;

    async fn from_parts_state_ref_pair(
        parts: &Parts,
        _state: &State,
    ) -> Result<Self, Self::Rejection> {
        let value = headers_to_json(
            parts
                .headers
                .iter()
                .map(|(n, v)| (n.as_str(), v.as_bytes())),
        );
        let parsed = crate::common::parse_coerced::<T>(&value).map_err(VldRejection::from_vld)?;
        Ok(VldHeaders(parsed))
    }
}
