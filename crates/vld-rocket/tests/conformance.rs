//! Shared HTTP conformance suite (see `vld_http_common::conformance`),
//! run with the routes mounted at `/` and at a non-root `/api/v1`.

use rocket::http::{Header, Method};
use rocket::local::asynchronous::Client;
use rocket::routes;
use vld_http_common::conformance::{
    Cookies, Headers, PathId, PathNone, PathStr, Query, Request, Response, User,
};
use vld_rocket::prelude::*;

#[rocket::post("/json", data = "<b>")]
fn json(b: VldJson<User>) -> String {
    b.reply()
}
#[rocket::post("/form", data = "<b>")]
fn form(b: VldForm<User>) -> String {
    b.reply()
}
#[rocket::get("/query")]
fn query(q: VldQuery<Query>) -> String {
    q.reply()
}
#[rocket::get("/path/<id>")]
fn path(id: &str, p: VldPath<PathId>) -> String {
    let _ = id;
    p.reply()
}
#[rocket::get("/pstr/<s>")]
fn pstr(s: &str, p: VldPath<PathStr>) -> String {
    let _ = s;
    p.s.clone()
}
#[rocket::get("/pnone")]
fn pnone(p: VldPath<PathNone>) -> String {
    p.reply()
}
#[rocket::get("/headers")]
fn headers(h: VldHeaders<Headers>) -> String {
    h.x_tag.clone()
}
#[rocket::get("/cookie")]
fn cookie(c: VldCookie<Cookies>) -> String {
    c.sid.clone()
}

async fn send_at(base: &str, r: Request) -> Response {
    let rocket = rocket::build()
        .mount(
            base,
            routes![json, form, query, path, pstr, pnone, headers, cookie],
        )
        .register("/", vld_catchers());
    let client = Client::untracked(rocket).await.expect("valid rocket");
    let method: Method = r.method.parse().unwrap();
    let mut req = client.req(method, r.uri);
    for (k, v) in r.headers {
        req.add_header(Header::new(k, String::from_utf8(v).unwrap()));
    }
    let resp = req.body(r.body).dispatch().await;
    Response {
        status: resp.status().code,
        body: resp.into_bytes().await.unwrap_or_default(),
    }
}

mod root {
    use super::*;

    async fn send(r: Request) -> Response {
        send_at("/", r).await
    }

    vld_http_common::conformance_suite! {
        runtime = rocket::async_test,
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
            cookie_missing,
        ],
    }
}

mod mounted {
    use super::*;

    async fn send(r: Request) -> Response {
        send_at("/api/v1", r).await
    }

    vld_http_common::conformance_suite! {
        runtime = rocket::async_test,
        send = send,
        prefix = "/api/v1",
        cases = [
            json_valid,
            json_validation_error,
            form_valid,
            query_percent_decoding,
            query_repeated_keys,
            path_valid,
            path_invalid,
            path_percent_decoding,
            path_no_double_decoding,
            path_no_params_optional,
            headers_valid,
            cookie_quoted_encoded,
        ],
    }
}
