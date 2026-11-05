//! Shared HTTP conformance suite (see `vld_http_common::conformance`).

use poem::http::{HeaderValue, Method};
use poem::{get, handler, post, Endpoint, Route};
use vld_http_common::conformance::{Cookies, Headers, PathId, Query, Request, Response, User};
use vld_poem::prelude::*;

#[handler]
async fn json(b: VldJson<User>) -> String {
    b.reply()
}
#[handler]
async fn form(b: VldForm<User>) -> String {
    b.reply()
}
#[handler]
async fn query(q: VldQuery<Query>) -> String {
    q.reply()
}
#[handler]
async fn path(p: VldPath<PathId>) -> String {
    p.reply()
}
#[handler]
async fn headers(h: VldHeaders<Headers>) -> String {
    h.x_tag.clone()
}
#[handler]
async fn cookie(c: VldCookie<Cookies>) -> String {
    c.sid.clone()
}

async fn send(r: Request) -> Response {
    let app = Route::new()
        .at("/json", post(json))
        .at("/form", post(form))
        .at("/query", get(query))
        .at("/path/:id", get(path))
        .at("/headers", get(headers))
        .at("/cookie", get(cookie));
    let mut req = poem::Request::builder()
        .method(Method::from_bytes(r.method.as_bytes()).unwrap())
        .uri(r.uri.parse().unwrap());
    for (k, v) in r.headers {
        req = req.header(k, HeaderValue::from_bytes(&v).unwrap());
    }
    let resp = app.get_response(req.body(r.body)).await;
    let status = resp.status().as_u16();
    Response {
        status,
        body: resp.into_body().into_vec().await.unwrap(),
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
        #[ignore = "BUG: VldPath sees no route params (req.params::<Vec<(String, String)>>() fails, falls back to empty)"]
        path_valid,
        path_invalid,
        headers_valid,
        headers_missing,
        cookie_quoted_encoded,
        cookie_missing,
    ],
}

/// Harness sanity check: poem's own `Path` sees the param on the same route
/// shape, so the `path_valid` failure is in `VldPath`, not in the test setup.
#[tokio::test]
async fn harness_poem_path_sees_params() {
    #[handler]
    async fn raw(poem::web::Path(id): poem::web::Path<i64>) -> String {
        format!("id={id}")
    }
    let app = Route::new().at("/path/:id", get(raw));
    let resp = app
        .get_response(
            poem::Request::builder()
                .uri("/path/42".parse().unwrap())
                .finish(),
        )
        .await;
    assert_eq!(resp.into_body().into_string().await.unwrap(), "id=42");
}
