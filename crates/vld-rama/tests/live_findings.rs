//! Regression tests for the live end-to-end findings (0.5.0).

use rama::http::body::util::BodyExt;
use rama::http::service::web::WebService;
use rama::http::{Body, Method, Request, StatusCode};
use rama::Service;
use vld_rama::{VldJson, VldQuery};

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

async fn send(req: Request) -> (StatusCode, String) {
    let svc = WebService::default()
        .with_post("/j", async |VldJson(n): VldJson<Named>| n.name)
        .with_get("/q", async |VldQuery(q): VldQuery<Search>| {
            format!("{}|{}|{}|{}", q.q, q.age, q.zip, q.tags.join(","))
        });
    let resp = svc.serve(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

fn json_req(ct: Option<&str>, body: &str) -> Request {
    let mut b = Request::builder()
        .method(Method::POST)
        .uri("http://example.com/j");
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
async fn query_coercion_follows_schema() {
    let req = Request::builder()
        .method(Method::GET)
        .uri("http://example.com/q?q=123&age=30&zip=01234&tags=a")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(req).await, (StatusCode::OK, "123|30|01234|a".into()));
}
