//! Regression tests for the 0.5.0 audit fixes.

use axum::body::Body;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::Router;
use http::{Request, StatusCode};
use tower::ServiceExt;
use vld_axum::{VldCookie, VldForm, VldHeaders, VldJson, VldQuery};

vld::schema! {
    #[derive(Debug)]
    pub struct Named {
        pub name: String => vld::string().min(2),
    }
}

async fn json_handler(body: VldJson<Named>) -> String {
    // Deref to the inner value.
    body.name.clone()
}

async fn form_handler(body: VldForm<Named>) -> String {
    body.name.clone()
}

vld::schema! {
    #[derive(Debug)]
    pub struct TagsQuery {
        pub tag: Vec<String> => vld::array(vld::string()),
        pub phone: String => vld::string(),
        pub empty: String => vld::string(),
    }
}

async fn tags_handler(q: VldQuery<TagsQuery>) -> String {
    format!("{}|{}|{}", q.tag.join(","), q.phone, q.empty)
}

vld::schema! {
    #[derive(Debug)]
    pub struct Sid {
        pub sid: String => vld::string(),
    }
}

async fn cookie_handler(c: VldCookie<Sid>) -> String {
    c.sid.clone()
}

vld::schema! {
    #[derive(Debug)]
    pub struct TagHeader {
        pub x_tag: String => vld::string(),
    }
}

async fn header_handler(h: VldHeaders<TagHeader>) -> String {
    h.x_tag.clone()
}

fn app() -> Router {
    Router::new()
        .route("/j", post(json_handler))
        .route("/f", post(form_handler))
        .route("/s", get(tags_handler))
        .route("/c", get(cookie_handler))
        .route("/h", get(header_handler))
        .layer(DefaultBodyLimit::max(16))
}

async fn send(req: Request<Body>) -> (StatusCode, String) {
    let resp = app().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

fn post_req(uri: &str, ct: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", ct)
        .body(Body::from(body.to_owned()))
        .unwrap()
}

#[tokio::test]
async fn json_too_large_returns_413_json() {
    let (status, body) = send(post_req(
        "/j",
        "application/json",
        r#"{"name":"a-very-long-name"}"#,
    ))
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(body, r#"{"error":"Payload too large"}"#);
}

#[tokio::test]
async fn form_too_large_returns_413_json() {
    let (status, body) = send(post_req(
        "/f",
        "application/x-www-form-urlencoded",
        "name=a-very-long-name",
    ))
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(body, r#"{"error":"Payload too large"}"#);
}

#[tokio::test]
async fn invalid_json_is_422_validation_shape() {
    let (status, body) = send(post_req("/j", "application/json", "{")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["error"], "Validation failed");
}

#[tokio::test]
async fn json_deref() {
    let (status, body) = send(post_req("/j", "application/json", r#"{"name":"Al"}"#)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "Al");
}

#[tokio::test]
async fn query_arrays_and_lossless_strings() {
    let req = Request::builder()
        .uri("/s?tag=a&tag=caf%C3%A9&phone=%2B15551234567&empty=")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(req).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, "a,café|+15551234567|");
}

#[tokio::test]
async fn cookie_quoted_value() {
    let req = Request::builder()
        .uri("/c")
        .header("cookie", "sid=\"abc\"")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(req).await, (StatusCode::OK, "abc".into()));
}

#[tokio::test]
async fn non_ascii_header_is_not_dropped() {
    let req = Request::builder()
        .uri("/h")
        .header(
            "x-tag",
            http::HeaderValue::from_bytes(b"caf\xC3\xA9").unwrap(),
        )
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(req).await, (StatusCode::OK, "café".into()));
}
