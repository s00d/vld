//! Regression tests for the live end-to-end findings (0.5.0).

use vld_warp::prelude::*;
use warp::Filter;

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

fn routes() -> impl Filter<Extract = (impl warp::Reply,), Error = std::convert::Infallible> + Clone
{
    let json = warp::post()
        .and(warp::path("j"))
        .and(vld_json::<Named>())
        .map(|n: Named| n.name);
    let query = warp::get()
        .and(warp::path("q"))
        .and(vld_query::<Search>())
        .map(|q: Search| format!("{}|{}|{}|{}", q.q, q.age, q.zip, q.tags.join(",")));
    json.or(query).recover(handle_rejection)
}

async fn post_json(ct: Option<&str>, body: &str) -> (u16, String) {
    let mut req = warp::test::request().method("POST").path("/j").body(body);
    if let Some(ct) = ct {
        req = req.header("content-type", ct);
    }
    let resp = req.reply(&routes()).await;
    (
        resp.status().as_u16(),
        String::from_utf8(resp.body().to_vec()).unwrap(),
    )
}

#[tokio::test]
async fn json_content_types() {
    for ct in [
        "application/json",
        "application/vnd.api+json",
        "application/problem+json; charset=utf-8",
    ] {
        assert_eq!(
            post_json(Some(ct), r#"{"name":"alice"}"#).await,
            (200, "alice".into()),
            "{ct}"
        );
    }
    for ct in [None, Some("text/plain")] {
        let (status, text) = post_json(ct, r#"{"name":"alice"}"#).await;
        assert_eq!(status, 415, "{ct:?}");
        let body: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(body["error"], "Unsupported Media Type");
    }
}

#[tokio::test]
async fn validation_issues_carry_code() {
    let (status, text) = post_json(Some("application/json"), r#"{"name":"a"}"#).await;
    assert_eq!(status, 422);
    let body: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(body["issues"][0]["code"], "too_small");
}

#[tokio::test]
async fn query_coercion_follows_schema() {
    let resp = warp::test::request()
        .path("/q?q=123&age=30&zip=01234&tags=a")
        .reply(&routes())
        .await;
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.body(), "123|30|01234|a");
}
