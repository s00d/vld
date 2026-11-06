//! Shared HTTP conformance suite (see `vld_http_common::conformance`).

use actix_web::{test, web, App, HttpResponse};
use vld_actix::{VldCookie, VldForm, VldHeaders, VldJson, VldPath, VldQuery};
use vld_http_common::conformance::{
    Cookies, Headers, PathId, PathNone, PathStr, Query, Request, Response, User,
};

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
async fn pstr(p: VldPath<PathStr>) -> HttpResponse {
    HttpResponse::Ok().body(p.s.clone())
}
async fn pnone(p: VldPath<PathNone>) -> HttpResponse {
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
            .route("/pstr/{s}", web::get().to(pstr))
            .route("/pnone", web::get().to(pnone))
            .route("/headers", web::get().to(headers))
            .route("/cookie", web::get().to(cookie)),
    )
    .await;
    let mut req = test::TestRequest::default()
        .method(r.method.parse().unwrap())
        .uri(&r.uri)
        .set_payload(r.body);
    for (k, v) in r.headers {
        req = req.append_header((
            k,
            actix_web::http::header::HeaderValue::from_bytes(&v).unwrap(),
        ));
    }
    let resp = test::call_service(&app, req.to_request()).await;
    Response {
        status: resp.status().as_u16(),
        body: test::read_body(resp).await.to_vec(),
    }
}

vld_http_common::conformance_suite! {
    runtime = actix_rt::test,
    send = send,
    cases = [
        json_valid,
        json_content_type_case_insensitive,
        json_content_type_bare_param,
        json_content_type_unterminated_quote,
        json_content_type_empty_suffix,
        json_content_type_json_suffix,
        json_content_type_json_suffix_params,
        json_content_type_missing,
        json_content_type_not_json,
        json_malformed,
        json_validation_error,
        json_oversized,
        form_valid,
        form_content_type_params,
        form_wrong_content_type,
        form_too_many_fields,
        form_validation_error,
        form_non_utf8,
        form_oversized,
        query_percent_decoding,
        query_repeated_keys,
        query_coercion_follows_schema,
        query_int_invalid,
        query_missing_field,
        path_valid,
        path_invalid,
        path_percent_decoding,
        path_no_double_decoding,
        path_invalid_utf8,
        path_no_params_optional,
        headers_valid,
        headers_underscore_ignored,
        headers_underscore_only,
        headers_missing,
        cookie_quoted_encoded,
        cookie_multiple_headers,
        cookie_duplicate_first_wins,
        cookie_non_ascii,
        cookie_numeric_value,
        cookie_missing,
    ],
}
