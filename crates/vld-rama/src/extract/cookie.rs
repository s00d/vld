use crate::rejection::VldRejection;
use rama::http::header;
use rama::http::request::Parts;
use rama::http::service::web::extract::FromPartsStateRefPair;
use vld::schema::VldParse;
use vld_http_common::cookie_headers_to_json;

/// Rama extractor that validates **cookie values** from the `Cookie`
/// headers (all of them; the first occurrence of a name wins).
pub struct VldCookie<T>(pub T);

impl<T, State> FromPartsStateRefPair<State> for VldCookie<T>
where
    T: VldParse + Send + Sync + 'static,
    State: Send + Sync + 'static,
{
    type Rejection = VldRejection;

    async fn from_parts_state_ref_pair(
        parts: &Parts,
        _state: &State,
    ) -> Result<Self, Self::Rejection> {
        let value = cookie_headers_to_json(
            parts
                .headers
                .get_all(header::COOKIE)
                .iter()
                .map(|v| v.as_bytes()),
        );
        let parsed = vld_http_common::parse_coerced::<T>(&value).map_err(VldRejection::from_vld)?;
        Ok(VldCookie(parsed))
    }
}
