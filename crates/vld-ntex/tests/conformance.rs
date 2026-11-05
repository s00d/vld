//! Shared HTTP conformance suite (see `vld_http_common::conformance`).
//! Runs on whichever of `ntex-3` / `ntex-4` is enabled.

mod common;
#[cfg(feature = "ntex-4")]
use common::TestRequestExt;
use ntex::http::header::HeaderValue;
use ntex::http::Method;
use ntex::web::{self, test, App, HttpResponse};
use vld_http_common::conformance::{Cookies, Headers, PathId, Query, Request, Response, User};
use vld_ntex::ntex;
use vld_ntex::{VldCookie, VldForm, VldHeaders, VldJson, VldPath, VldQuery};

async fn json(b: VldJson<User>) -> HttpResponse {
    HttpResponse::Ok().body(b.reply())
}
async fn form(b: VldForm<User>) -> HttpResponse {
    HttpResponse::Ok().body(b.reply())
}
async fn query(q: VldQuery<Query>) -> HttpResponse {
    HttpResponse::Ok().body(q.reply())
}
async fn path(p: VldPath<PathId>) -> HttpResponse {
    HttpResponse::Ok().body(p.reply())
}
async fn headers(h: VldHeaders<Headers>) -> HttpResponse {
    HttpResponse::Ok().body(h.x_tag.clone())
}
async fn cookie(c: VldCookie<Cookies>) -> HttpResponse {
    HttpResponse::Ok().body(c.sid.clone())
}

async fn send(r: Request) -> Response {
    let app = test::init_service(
        App::new()
            .route("/json", web::post().to(json))
            .route("/form", web::post().to(form))
            .route("/query", web::get().to(query))
            .route("/path/{id}", web::get().to(path))
            .route("/headers", web::get().to(headers))
            .route("/cookie", web::get().to(cookie)),
    )
    .await;
    let mut req = test::TestRequest::default()
        .uri(&r.uri)
        .method(Method::from_bytes(r.method.as_bytes()).unwrap())
        .set_payload(r.body);
    for (k, v) in r.headers {
        req = req.header(k, HeaderValue::from_bytes(&v).unwrap());
    }
    let resp = test::call_service(&app, req.to_request()).await;
    Response {
        status: resp.status().as_u16(),
        body: test::read_body(resp).await.to_vec(),
    }
}

vld_http_common::conformance_suite! {
    runtime = ntex::test,
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
