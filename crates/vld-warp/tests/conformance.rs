//! Shared HTTP conformance suite (see `vld_http_common::conformance`).

use http_body_util::{BodyExt, Full};
use tower::ServiceExt;
use vld_http_common::conformance::{
    Cookies, Headers, PathId, PathNone, PathStr, Query, Request, Response, User,
};
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
    let pstr = warp::get()
        .and(warp::path("pstr"))
        .and(vld_param::<PathStr>("s"))
        .and(warp::path::end())
        .map(|p: PathStr| p.s);
    let pnone = warp::get()
        .and(warp::path("pnone"))
        .and(vld_path::<PathNone>(&[]))
        .map(|p: PathNone| p.reply());
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
        .or(pstr)
        .or(pnone)
        .or(headers)
        .or(cookie)
        .recover(handle_rejection)
}

/// Through `warp::service`: `warp::test::request()` replaces repeated
/// headers instead of appending them.
async fn send(r: Request) -> Response {
    let mut req = warp::http::Request::builder().method(r.method).uri(&r.uri);
    for (k, v) in r.headers {
        req = req.header(k, HeaderValue::from_bytes(&v).unwrap());
    }
    let req = req.body(Full::new(bytes::Bytes::from(r.body))).unwrap();
    let resp = warp::service(routes()).oneshot(req).await.unwrap();
    Response {
        status: resp.status().as_u16(),
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
        json_content_type_empty_suffix,
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
        query_missing_field,
        path_valid,
        path_invalid,
        path_percent_decoding,
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
        cookie_missing,
    ],
}
