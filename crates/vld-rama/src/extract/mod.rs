mod cookie;
mod form;
mod headers;
mod json;
mod path;
mod query;

pub use cookie::VldCookie;
pub use form::VldForm;
pub use headers::VldHeaders;
pub use json::VldJson;
pub use path::VldPath;
pub use query::VldQuery;

use crate::rejection::VldRejection;
use crate::VldBodyLimit;

use rama::http::body::util::BodyExt;
use rama::http::request::Parts;
use rama::http::Body;
use vld_http_common::DEFAULT_BODY_LIMIT;

vld_http_common::impl_deref!(VldJson, VldQuery, VldPath, VldForm, VldHeaders, VldCookie);

/// The [`VldBodyLimit`] extension, or [`DEFAULT_BODY_LIMIT`].
fn body_limit(parts: &Parts) -> usize {
    parts
        .extensions
        .get_ref::<VldBodyLimit>()
        .map_or(DEFAULT_BODY_LIMIT, |l| l.0)
}

async fn read_body(body: Body, limit: usize) -> Result<rama::bytes::Bytes, VldRejection> {
    Ok(body
        .limited(limit)
        .collect()
        .await
        .map_err(|e| VldRejection::from_body_error(&e))?
        .to_bytes())
}
