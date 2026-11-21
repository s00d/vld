//! Stories of a Lambda function validating what API Gateway and raw
//! invocations hand it.

use std::collections::HashMap;

use lambda_http::lambda_runtime::{Context, Diagnostic, LambdaEvent};
use lambda_http::{Body, Request, RequestExt};
use serde_json::{Value, json};
use vld::error::{IssueCode, VldError};
use vld_web::common::VldHttpError;
use vld_web::lambda::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct Search {
        pub q: String => vld::string().min(1),
        pub tags: Vec<String> => vld::array(vld::string()),
        pub limit: Option<i64> => vld::number().int().min(1).max(100).optional(),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct Order {
        pub id: String => vld::string().min(1),
        pub qty: i64 => vld::number().int().min(1),
    }
}

/// Status, `Content-Type` and JSON body of an error response.
fn parts(resp: &lambda_http::Response<Body>) -> (u16, &str, Value) {
    (
        resp.status().as_u16(),
        resp.headers()["content-type"].to_str().unwrap(),
        serde_json::from_slice(resp.body()).unwrap(),
    )
}

fn get() -> Request {
    lambda_http::http::Request::builder()
        .uri("/search")
        .body(Body::Empty)
        .unwrap()
}

/// Given a search endpoint behind API Gateway,
/// when the event carries multi-value `queryStringParameters`,
/// then repeated keys fill the array, numbers are coerced, and a bad limit
/// is a 422 JSON response naming `.limit`.
#[test]
fn a_search_endpoint_validates_its_query_parameters() {
    let req = get().with_query_string_parameters(HashMap::from([
        ("q".to_string(), vec!["rust & co".to_string()]),
        ("tags".to_string(), vec!["a=b".to_string(), "c".to_string()]),
        ("limit".to_string(), vec!["20".to_string()]),
    ]));
    let s: Search = validate_query(&req).unwrap();
    assert_eq!(
        (s.q.as_str(), s.tags, s.limit),
        (
            "rust & co",
            vec!["a=b".to_string(), "c".to_string()],
            Some(20)
        )
    );

    let req = get().with_query_string_parameters(HashMap::from([
        ("q".to_string(), "x".to_string()),
        ("limit".to_string(), "500".to_string()),
    ]));
    let resp = validate_query::<Search>(&req).unwrap_err();
    let (status, ct, body) = parts(&resp);
    assert_eq!((status, ct), (422, "application/json"));
    assert_eq!(body["error"], "Validation failed");
    let issues: Vec<(&str, &str)> = body["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| (i["path"].as_str().unwrap(), i["code"].as_str().unwrap()))
        .collect();
    assert_eq!(issues, [(".tags", "missing_field"), (".limit", "too_big")]);

    // No parameters at all: every required field is missing.
    let resp = validate_query::<Search>(&get()).unwrap_err();
    assert_eq!(parts(&resp).2["issues"].as_array().unwrap().len(), 2);
}

/// Given a handler that answers errors it builds itself,
/// when it turns a `VldError` or a 415 into a response,
/// then the status and body are those of every HTTP module.
#[test]
fn a_handler_builds_error_responses_by_hand() {
    let err = VldError::single(
        IssueCode::Custom {
            code: "taken".into(),
        },
        "Name is taken",
    );
    let resp = error_response(&err.into());
    assert_eq!(
        parts(&resp),
        (
            422,
            "application/json",
            json!({"error": "Validation failed", "issues": [
                {"path": "", "message": "Name is taken", "code": "taken"}
            ]})
        )
    );

    let resp = error_response(&VldHttpError::unsupported_media_type());
    let (status, _, body) = parts(&resp);
    assert_eq!(
        (status, body["error"].as_str()),
        (415, Some("Unsupported Media Type"))
    );
}

/// Given an SQS-style function receiving raw JSON events,
/// when a valid and an invalid order arrive,
/// then the valid one comes back with its context, and the invalid one is a
/// `ValidationError` diagnostic whose message is the 422 body.
#[test]
fn a_raw_event_function_validates_its_payload() {
    let mut context = Context::default();
    context.request_id = "req-1".into();

    let (order, ctx): (Order, Context) = validate_event(LambdaEvent::new(
        json!({"id": "o-1", "qty": 2}),
        context.clone(),
    ))
    .unwrap();
    assert_eq!(
        (order.id.as_str(), order.qty, ctx.request_id.as_str()),
        ("o-1", 2, "req-1")
    );

    let diag: Diagnostic =
        validate_event::<Order>(LambdaEvent::new(json!({"id": "", "qty": 0}), context))
            .unwrap_err();
    assert_eq!(diag.error_type, "ValidationError");
    let body: Value = serde_json::from_str(&diag.error_message).unwrap();
    assert_eq!(body["error"], "Validation failed");
    let issues: Vec<(&str, &str)> = body["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| (i["path"].as_str().unwrap(), i["code"].as_str().unwrap()))
        .collect();
    assert_eq!(issues, [(".id", "too_small"), (".qty", "too_small")]);
}
