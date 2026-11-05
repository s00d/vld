//! Regression tests for the live end-to-end findings (0.5.0).

use axum::body::Body;
use axum::routing::{get, post};
use axum::Router;
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;
use vld_axum::{VldCookie, VldJson, VldQuery};

vld::schema! {
    #[derive(Debug)]
    pub struct Named {
        pub name: String => vld::string().min(2),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct Search {
        pub q: String => vld::string(),
        pub age: i64 => vld::number().int(),
        pub zip: String => vld::string(),
        pub tags: Vec<String> => vld::array(vld::string()),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct Session {
        pub session: String => vld::string(),
    }
}

fn app() -> Router {
    Router::new()
        .route(
            "/j",
            post(|b: VldJson<Named>| async move { b.name.clone() }),
        )
        .route(
            "/q",
            get(|q: VldQuery<Search>| async move {
                format!("{}|{}|{}|{}", q.q, q.age, q.zip, q.tags.join(","))
            }),
        )
        .route(
            "/c",
            get(|c: VldCookie<Session>| async move { c.session.clone() }),
        )
}

async fn send(req: Request<Body>) -> (StatusCode, String) {
    let resp = app().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

fn json_req(ct: Option<&str>, body: &str) -> Request<Body> {
    let mut b = Request::post("/j");
    if let Some(ct) = ct {
        b = b.header("content-type", ct);
    }
    b.body(Body::from(body.to_string())).unwrap()
}

#[tokio::test]
async fn json_content_types() {
    for ct in [
        "application/json",
        "application/vnd.api+json",
        "application/problem+json; charset=utf-8",
    ] {
        let (status, text) = send(json_req(Some(ct), r#"{"name":"alice"}"#)).await;
        assert_eq!((status, text.as_str()), (StatusCode::OK, "alice"), "{ct}");
    }
    for ct in [None, Some("text/plain")] {
        let (status, text) = send(json_req(ct, r#"{"name":"alice"}"#)).await;
        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE, "{ct:?}");
        let body: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(body["error"], "Unsupported Media Type");
    }
}

#[tokio::test]
async fn validation_issues_carry_code() {
    let (status, text) = send(json_req(Some("application/json"), r#"{"name":"a"}"#)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(body["issues"][0]["code"], "too_small");
}

#[tokio::test]
async fn coercion_follows_schema() {
    let req = Request::get("/q?q=123&age=30&zip=01234&tags=a")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(req).await, (StatusCode::OK, "123|30|01234|a".into()));

    let req = Request::get("/c")
        .header("cookie", "session=42")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(req).await, (StatusCode::OK, "42".into()));
}
