//! Regression tests for the 0.5.0 audit fixes.

use poem::test::TestClient;
use poem::{handler, post, Route};
use vld_poem::prelude::*;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Named {
        pub name: String => vld::string().min(2),
    }
}

#[handler]
async fn json_handler(body: VldJson<Named>) -> String {
    body.name.clone()
}

#[handler]
async fn form_handler(body: VldForm<Named>) -> String {
    body.name.clone()
}

/// Reads the body twice: the second extractor finds it already taken.
#[handler]
async fn double_body(_a: VldJson<Named>, _b: VldJson<Named>) -> String {
    "unreachable".into()
}

fn app() -> Route {
    Route::new()
        .at("/j", post(json_handler))
        .at("/f", post(form_handler))
        .at("/twice", post(double_body))
}

/// The body limit is configurable with `VldBodyLimit` request data
/// (it was hard-coded to 2 MiB).
#[tokio::test]
async fn body_limit_from_request_data() {
    use poem::EndpointExt;
    let small = TestClient::new(Route::new().at("/j", post(json_handler).data(VldBodyLimit(8))));
    small
        .post("/j")
        .content_type("application/json")
        .body(r#"{"name":"Alice"}"#)
        .send()
        .await
        .assert_status(poem::http::StatusCode::PAYLOAD_TOO_LARGE);

    let big = format!(r#"{{"name":"{}"}}"#, "x".repeat(3 << 20));
    let large =
        TestClient::new(Route::new().at("/j", post(json_handler).data(VldBodyLimit(4 << 20))));
    large
        .post("/j")
        .content_type("application/json")
        .body(big)
        .send()
        .await
        .assert_status_is_ok();
}

#[tokio::test]
async fn json_too_large_returns_413_json() {
    let big = format!(
        r#"{{"name":"{}"}}"#,
        "x".repeat(vld_http_common::DEFAULT_BODY_LIMIT)
    );
    let resp = TestClient::new(app())
        .post("/j")
        .content_type("application/json")
        .body(big)
        .send()
        .await;
    resp.assert_status(poem::http::StatusCode::PAYLOAD_TOO_LARGE);
    resp.assert_text(r#"{"error":"Payload too large"}"#).await;
}

#[tokio::test]
async fn form_too_large_returns_413_json() {
    let big = format!("name={}", "x".repeat(vld_http_common::DEFAULT_BODY_LIMIT));
    let resp = TestClient::new(app())
        .post("/f")
        .content_type("application/x-www-form-urlencoded")
        .body(big)
        .send()
        .await;
    resp.assert_status(poem::http::StatusCode::PAYLOAD_TOO_LARGE);
    resp.assert_text(r#"{"error":"Payload too large"}"#).await;
}

#[tokio::test]
async fn body_taken_twice_is_json_error() {
    let resp = TestClient::new(app())
        .post("/twice")
        .content_type("application/json")
        .body(r#"{"name":"Al"}"#)
        .send()
        .await;
    resp.assert_status(poem::http::StatusCode::UNPROCESSABLE_ENTITY);
    resp.assert_content_type("application/json");
}

#[tokio::test]
async fn non_utf8_form_is_422_validation_shape() {
    let resp = TestClient::new(app())
        .post("/f")
        .content_type("application/x-www-form-urlencoded")
        .body(b"name=\xFF\xFE".to_vec())
        .send()
        .await;
    resp.assert_status(poem::http::StatusCode::UNPROCESSABLE_ENTITY);
    let json: serde_json::Value = resp.0.into_body().into_json().await.unwrap();
    assert_eq!(json["error"], "Validation failed");
}

#[test]
fn display_is_human_readable() {
    let err: VldPoemError =
        vld::error::VldError::single(vld::error::IssueCode::MissingField, "Required").into();
    assert!(err.to_string().starts_with("Validation failed"));
    assert_eq!(err.body()["error"], "Validation failed");
}

mod path_from_route {
    use poem::test::TestClient;
    use poem::{handler, Route};
    use vld_poem::prelude::*;
    vld::schema! {
        #[derive(Debug, Clone)]
        pub struct UserPath {
            pub id: i64 => vld::number().int().min(1),
        }
    }

    #[handler]
    async fn get_user(p: VldPath<UserPath>) -> String {
        p.id.to_string()
    }

    #[tokio::test]
    async fn path_param_comes_from_route() {
        let cli = TestClient::new(Route::new().at("/users/:id", poem::get(get_user)));
        let resp = cli.get("/users/42").send().await;
        resp.assert_status_is_ok();
        resp.assert_text("42").await;
    }

    #[tokio::test]
    async fn path_param_cannot_be_overridden_by_query() {
        let cli = TestClient::new(Route::new().at("/users/:id", poem::get(get_user)));
        let resp = cli.get("/users/42").query("id", &7).send().await;
        resp.assert_status_is_ok();
        resp.assert_text("42").await;
    }
}
