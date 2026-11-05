//! Regression tests for the live end-to-end findings (0.5.0).

use poem::http::StatusCode;
use poem::test::TestClient;
use poem::{get, handler, post, Route};
use vld_poem::prelude::*;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Named {
        pub name: String => vld::string().min(2),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Search {
        pub q: String => vld::string(),
        pub age: i64 => vld::number().int(),
        pub zip: String => vld::string(),
        pub tags: Vec<String> => vld::array(vld::string()),
    }
}

#[handler]
async fn json_handler(body: VldJson<Named>) -> String {
    body.name.clone()
}

#[handler]
async fn query_handler(q: VldQuery<Search>) -> String {
    format!("{}|{}|{}|{}", q.q, q.age, q.zip, q.tags.join(","))
}

fn app() -> Route {
    Route::new()
        .at("/j", post(json_handler))
        .at("/q", get(query_handler))
}

#[tokio::test]
async fn json_content_types() {
    let cli = TestClient::new(app());
    for ct in [
        "application/json",
        "application/vnd.api+json",
        "application/problem+json; charset=utf-8",
    ] {
        let resp = cli
            .post("/j")
            .content_type(ct)
            .body(r#"{"name":"alice"}"#)
            .send()
            .await;
        resp.assert_status_is_ok();
        resp.assert_text("alice").await;
    }
    for ct in [None, Some("text/plain")] {
        let mut req = cli.post("/j").body(r#"{"name":"alice"}"#);
        if let Some(ct) = ct {
            req = req.content_type(ct);
        }
        let resp = req.send().await;
        resp.assert_status(StatusCode::UNSUPPORTED_MEDIA_TYPE);
        let body: serde_json::Value = resp.0.into_body().into_json().await.unwrap();
        assert_eq!(body["error"], "Unsupported Media Type");
    }
}

#[tokio::test]
async fn validation_issues_carry_code() {
    let resp = TestClient::new(app())
        .post("/j")
        .content_type("application/json")
        .body(r#"{"name":"a"}"#)
        .send()
        .await;
    resp.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = resp.0.into_body().into_json().await.unwrap();
    assert_eq!(body["issues"][0]["code"], "too_small");
}

#[tokio::test]
async fn query_coercion_follows_schema() {
    let resp = TestClient::new(app())
        .get("/q?q=123&age=30&zip=01234&tags=a")
        .send()
        .await;
    resp.assert_status_is_ok();
    resp.assert_text("123|30|01234|a").await;
}
