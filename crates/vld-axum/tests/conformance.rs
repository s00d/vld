//! Shared HTTP conformance suite (see `vld_http_common::conformance`).

use axum::body::Body;
use axum::routing::{get, post};
use axum::Router;
use tower::ServiceExt;
use vld_axum::{VldCookie, VldForm, VldHeaders, VldJson, VldPath, VldQuery};
use vld_http_common::conformance::{Cookies, Headers, PathId, Query, Request, Response, User};

async fn json(b: VldJson<User>) -> String {
    b.reply()
}
async fn form(b: VldForm<User>) -> String {
    b.reply()
}
async fn query(q: VldQuery<Query>) -> String {
    q.reply()
}
async fn path(p: VldPath<PathId>) -> String {
    p.reply()
}
async fn headers(h: VldHeaders<Headers>) -> String {
    h.x_tag.clone()
}
async fn cookie(c: VldCookie<Cookies>) -> String {
    c.sid.clone()
}

async fn send(r: Request) -> Response {
    let app = Router::new()
        .route("/json", post(json))
        .route("/form", post(form))
        .route("/query", get(query))
        .route("/path/{id}", get(path))
        .route("/headers", get(headers))
        .route("/cookie", get(cookie));
    let mut req = http::Request::builder().method(r.method).uri(&r.uri);
    for (k, v) in r.headers {
        req = req.header(k, http::HeaderValue::from_bytes(&v).unwrap());
    }
    let resp = app
        .oneshot(req.body(Body::from(r.body)).unwrap())
        .await
        .unwrap();
    let status = resp.status().as_u16();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    Response {
        status,
        body: body.to_vec(),
    }
}

vld_http_common::conformance_suite! {
    runtime = tokio::test,
    send = send,
    cases = [
        json_valid,
        json_content_type_case_insensitive,
        json_malformed,
        json_validation_error,
        json_oversized,
        form_valid,
        form_validation_error,
        form_non_utf8,
        form_oversized,
        query_percent_decoding,
        query_repeated_keys,
        query_missing_field,
        path_valid,
        path_invalid,
        headers_valid,
        headers_missing,
        cookie_quoted_encoded,
        cookie_missing,
    ],
}
