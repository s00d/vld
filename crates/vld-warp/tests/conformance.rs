//! Shared HTTP conformance suite (see `vld_http_common::conformance`).

use vld_http_common::conformance::{Cookies, Headers, PathId, Query, Request, Response, User};
use vld_warp::prelude::*;
use warp::http::HeaderValue;
use warp::Filter;

fn routes() -> impl Filter<Extract = (impl warp::Reply,), Error = std::convert::Infallible> + Clone
{
    let json = warp::post()
        .and(warp::path!("json"))
        .and(vld_json::<User>())
        .map(|u: User| u.reply());
    let form = warp::post()
        .and(warp::path!("form"))
        .and(vld_form::<User>())
        .map(|u: User| u.reply());
    let query = warp::get()
        .and(warp::path!("query"))
        .and(vld_query::<Query>())
        .map(|q: Query| q.reply());
    let path = warp::get()
        .and(warp::path("path"))
        .and(vld_param::<PathId>("id"))
        .and(warp::path::end())
        .map(|p: PathId| p.reply());
    let headers = warp::get()
        .and(warp::path!("headers"))
        .and(vld_headers::<Headers>())
        .map(|h: Headers| h.x_tag);
    let cookie = warp::get()
        .and(warp::path!("cookie"))
        .and(vld_cookie::<Cookies>())
        .map(|c: Cookies| c.sid);
    json.or(form)
        .or(query)
        .or(path)
        .or(headers)
        .or(cookie)
        .recover(handle_rejection)
}

async fn send(r: Request) -> Response {
    let mut req = warp::test::request().method(r.method).path(&r.uri);
    for (k, v) in r.headers {
        req = req.header(k, HeaderValue::from_bytes(&v).unwrap());
    }
    let resp = req.body(r.body).reply(&routes()).await;
    Response {
        status: resp.status().as_u16(),
        body: resp.body().to_vec(),
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
