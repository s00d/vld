use crate::rejection::VldRejection;
use rama::http::matcher::UriParams;
use rama::http::request::Parts;
use rama::http::service::web::extract::FromPartsStateRefPair;
use vld::schema::VldParse;
use vld_http_common::path_params_to_json;

/// Rama extractor that validates **URL path parameters**.
///
/// Path segment values are coerced the same way as query parameters.
pub struct VldPath<T>(pub T);

impl<T, State> FromPartsStateRefPair<State> for VldPath<T>
where
    T: VldParse + Send + Sync + 'static,
    State: Send + Sync + 'static,
{
    type Rejection = VldRejection;

    async fn from_parts_state_ref_pair(
        parts: &Parts,
        _state: &State,
    ) -> Result<Self, Self::Rejection> {
        let params = parts
            .extensions
            .get_ref::<UriParams>()
            .ok_or_else(|| VldRejection::parse("No path parameters found for matched route"))?;

        let value = path_params_to_json(params.iter());
        let parsed = T::vld_parse_value(&value).map_err(VldRejection::from_vld)?;
        Ok(VldPath(parsed))
    }
}
