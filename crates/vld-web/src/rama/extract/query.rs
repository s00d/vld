use crate::common::query_string_to_json;
use crate::rama::rejection::VldRejection;
use rama::http::request::Parts;
use rama::http::service::web::extract::FromPartsStateRefPair;
use vld::schema::VldParse;

/// Rama extractor that validates **URL query parameters**.
///
/// See [`crate::common::parse_query_string`]: repeated keys and `a[]`
/// become arrays, values are coerced losslessly (`"42"` → number,
/// `"true"`/`"false"` → boolean, `?flag` / `?x=` → `""`, `"01234"` stays a
/// string).
pub struct VldQuery<T>(pub T);

impl<T, State> FromPartsStateRefPair<State> for VldQuery<T>
where
    T: VldParse + Send + Sync + 'static,
    State: Send + Sync + 'static,
{
    type Rejection = VldRejection;

    async fn from_parts_state_ref_pair(
        parts: &Parts,
        _state: &State,
    ) -> Result<Self, Self::Rejection> {
        let query_string = parts
            .uri
            .query()
            .map(|q| q.as_encoded_str().into_owned())
            .unwrap_or_default();
        let value = query_string_to_json(&query_string);
        let parsed = crate::common::parse_coerced::<T>(&value).map_err(VldRejection::from_vld)?;
        Ok(VldQuery(parsed))
    }
}
