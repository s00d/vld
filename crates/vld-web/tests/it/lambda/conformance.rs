//! Shared HTTP conformance suite (see `crate::conformance`).
//!
//! `send` plays API Gateway: it decodes the query string into
//! `queryStringParameters` and matches the path against the routes into
//! (decoded) `pathParameters`, then calls the module's functions. There is
//! no body limit to test (Lambda caps payloads itself), and the module has
//! no form / header / cookie helpers.

use std::collections::HashMap;

use crate::conformance::{PathId, PathNone, PathStr, Query, Request, Response, User};
use lambda_http::{Body, RequestExt};
use vld_web::common::{percent_decode, url_decode};
use vld_web::lambda::{validate_json, validate_path, validate_query};

fn reply(result: Result<String, lambda_http::Response<Body>>) -> Response {
    match result {
        Ok(text) => Response {
            status: 200,
            body: text.into_bytes(),
        },
        Err(resp) => Response {
            status: resp.status().as_u16(),
            body: resp.body().to_vec(),
        },
    }
}

async fn send(r: Request) -> Response {
    let (path, query) = r.uri.split_once('?').unwrap_or((&r.uri, ""));
    let mut params: HashMap<String, Vec<String>> = HashMap::new();
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        params.entry(url_decode(k)).or_default().push(url_decode(v));
    }
    let segments: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let path_params: HashMap<String, String> = match segments.as_slice() {
        ["path", id] => HashMap::from([("id".to_string(), percent_decode(id))]),
        ["pstr", s] => HashMap::from([("s".to_string(), percent_decode(s))]),
        _ => HashMap::new(),
    };

    let mut req = lambda_http::http::Request::builder()
        .method(r.method)
        .uri(path);
    for (k, v) in r.headers {
        req = req.header(k, v);
    }
    let req = req
        .body(Body::from(r.body))
        .unwrap()
        .with_query_string_parameters(params)
        .with_path_parameters(path_params);

    reply(match segments[0] {
        "json" => validate_json::<User>(&req).map(|u| u.reply()),
        "query" => validate_query::<Query>(&req).map(|q| q.reply()),
        "path" => validate_path::<PathId>(&req).map(|p| p.reply()),
        "pstr" => validate_path::<PathStr>(&req).map(|p| p.s),
        "pnone" => validate_path::<PathNone>(&req).map(|p| p.reply()),
        other => panic!("no route {other}"),
    })
}

conformance_suite! {
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
    ],
}
