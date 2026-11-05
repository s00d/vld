//! Regression tests for the live end-to-end findings (0.5.0).

use salvo::prelude::*;
use salvo::test::{ResponseExt, TestClient};
use vld_salvo::prelude::*;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Named {
        pub name: String => vld::string().min(2),
    }
}

#[handler]
async fn json_handler(b: VldJson<Named>, res: &mut Response) {
    res.render(b.name.clone());
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
async fn query_handler(q: VldQuery<Search>, res: &mut Response) {
    res.render(format!("{}|{}|{}|{}", q.q, q.age, q.zip, q.tags.join(",")));
}

fn service() -> Service {
    Service::new(
        Router::new()
            .push(Router::with_path("json").post(json_handler))
            .push(Router::with_path("q").get(query_handler)),
    )
}

async fn post_json(ct: Option<&str>, body: &str) -> (StatusCode, serde_json::Value, String) {
    let mut req = TestClient::post("http://localhost/json").body(body.to_string());
    if let Some(ct) = ct {
        req = req.add_header("content-type", ct, true);
    }
    let mut resp = req.send(&service()).await;
    let status = resp.status_code.unwrap();
    let text = resp.take_string().await.unwrap();
    (
        status,
        serde_json::from_str(&text).unwrap_or_default(),
        text,
    )
}

#[tokio::test]
async fn json_suffix_content_types_are_accepted() {
    for ct in [
        "application/json",
        "application/vnd.api+json",
        "application/problem+json; charset=utf-8",
        "Application/JSON",
    ] {
        let (status, _, text) = post_json(Some(ct), r#"{"name":"alice"}"#).await;
        assert_eq!(status, StatusCode::OK, "{ct}");
        assert_eq!(text, "alice");
    }
}

#[tokio::test]
async fn missing_or_non_json_content_type_is_415() {
    for ct in [None, Some("text/plain")] {
        let (status, body, _) = post_json(ct, r#"{"name":"alice"}"#).await;
        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE, "{ct:?}");
        assert_eq!(body["error"], "Unsupported Media Type");
    }
}

#[tokio::test]
async fn validation_issues_carry_code() {
    let (status, body, _) = post_json(Some("application/json"), r#"{"name":"a"}"#).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["issues"][0]["code"], "too_small");
    assert_eq!(body["issues"][0]["path"], ".name");
}

#[tokio::test]
async fn query_coercion_follows_schema() {
    let mut resp = TestClient::get("http://localhost/q?q=123&age=30&zip=01234&tags=a")
        .send(&service())
        .await;
    assert_eq!(resp.status_code.unwrap(), StatusCode::OK);
    assert_eq!(resp.take_string().await.unwrap(), "123|30|01234|a");
}
