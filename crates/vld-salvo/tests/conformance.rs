//! Shared HTTP conformance suite (see `vld_http_common::conformance`).

use salvo::http::{HeaderValue, Method};
use salvo::prelude::*;
use salvo::test::{RequestBuilder, ResponseExt};
use vld_http_common::conformance::{
    self as conf, Cookies, Headers, PathId, PathNone, PathStr, Query, User,
};
use vld_salvo::prelude::*;

#[handler]
async fn json(b: VldJson<User>, res: &mut salvo::Response) {
    res.render(b.reply());
}
#[handler]
async fn form(b: VldForm<User>, res: &mut salvo::Response) {
    res.render(b.reply());
}
#[handler]
async fn query(q: VldQuery<Query>, res: &mut salvo::Response) {
    res.render(q.reply());
}
#[handler]
async fn path(p: VldPath<PathId>, res: &mut salvo::Response) {
    res.render(p.reply());
}
#[handler]
async fn pstr(p: VldPath<PathStr>, res: &mut salvo::Response) {
    res.render(p.s.clone());
}
#[handler]
async fn pnone(p: VldPath<PathNone>, res: &mut salvo::Response) {
    res.render(p.reply());
}
#[handler]
async fn headers(h: VldHeaders<Headers>, res: &mut salvo::Response) {
    res.render(h.x_tag.clone());
}
#[handler]
async fn cookie(c: VldCookie<Cookies>, res: &mut salvo::Response) {
    res.render(c.sid.clone());
}

async fn send(r: conf::Request) -> conf::Response {
    let service = Service::new(
        Router::new()
            .push(Router::with_path("json").post(json))
            .push(Router::with_path("form").post(form))
            .push(Router::with_path("query").get(query))
            .push(Router::with_path("path/{id}").get(path))
            .push(Router::with_path("pstr/{s}").get(pstr))
            .push(Router::with_path("pnone").get(pnone))
            .push(Router::with_path("headers").get(headers))
            .push(Router::with_path("cookie").get(cookie)),
    );
    let mut req = RequestBuilder::new(
        format!("http://localhost{}", r.uri),
        Method::from_bytes(r.method.as_bytes()).unwrap(),
    );
    for (k, v) in r.headers {
        req = req.add_header(k, HeaderValue::from_bytes(&v).unwrap(), false);
    }
    let mut resp = req.body(r.body).send(&service).await;
    conf::Response {
        status: resp.status_code.map_or(200, |s| s.as_u16()),
        body: resp.take_bytes(None).await.unwrap().to_vec(),
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
