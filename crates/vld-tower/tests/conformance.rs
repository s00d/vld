//! Shared HTTP conformance suite (see `vld_http_common::conformance`).
//!
//! `ValidateJsonLayer` only validates JSON bodies, so only the JSON cases
//! apply; the inner service answers `/json` with the validated user.

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use tower::{ServiceBuilder, ServiceExt};
use vld_http_common::conformance::{Request, Response, User};
use vld_tower::{validated, ValidateJsonLayer, ValidatedBody};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

async fn inner(
    req: http::Request<ValidatedBody<Full<Bytes>>>,
) -> Result<http::Response<Full<Bytes>>, BoxError> {
    let reply = validated::<User>(&req).map_or_else(|| "not validated".into(), |u| u.reply());
    Ok(http::Response::new(Full::new(Bytes::from(reply))))
}

async fn send(r: Request) -> Response {
    let svc = ServiceBuilder::new()
        .layer(ValidateJsonLayer::<User>::new())
        .service_fn(inner);
    let mut req = http::Request::builder().method(r.method).uri(&r.uri);
    for (k, v) in r.headers {
        req = req.header(k, http::HeaderValue::from_bytes(&v).unwrap());
    }
    let resp = svc
        .oneshot(req.body(Full::new(Bytes::from(r.body))).unwrap())
        .await
        .unwrap();
    let status = resp.status().as_u16();
    Response {
        status,
        body: resp
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    }
}

vld_http_common::conformance_suite! {
    runtime = tokio::test,
    send = send,
    cases = [
        json_valid,
        json_content_type_case_insensitive,
        json_content_type_bare_param,
        json_content_type_unterminated_quote,
        json_malformed,
        json_validation_error,
        json_oversized,
    ],
}
