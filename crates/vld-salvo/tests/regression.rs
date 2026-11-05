//! Regression tests for the 0.5.0 audit fixes.

use salvo::prelude::*;
use salvo::test::{ResponseExt, TestClient};
use vld_salvo::prelude::*;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Login {
        pub username: String => vld::string().min(3),
        pub age: i64 => vld::number().int().min(0),
        pub tags: Vec<String> => vld::array(vld::string()),
    }
}

#[handler]
async fn form_handler(f: VldForm<Login>, res: &mut Response) {
    res.render(format!("{}:{}:{}", f.username, f.age, f.tags.join(",")));
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Named {
        pub name: String => vld::string(),
    }
}

#[handler]
async fn json_handler(b: VldJson<Named>, res: &mut Response) {
    res.render(b.name.clone());
}

fn service() -> Service {
    Service::new(
        Router::new()
            .push(Router::with_path("form").post(form_handler))
            .push(Router::with_path("json").post(json_handler)),
    )
}

/// The routes behind an explicit 1 KiB `SecureMaxSize`.
fn limited_service() -> Service {
    use salvo::http::request::SecureMaxSize;
    Service::new(
        Router::new()
            .hoop(SecureMaxSize::new(1024))
            .push(Router::with_path("form").post(form_handler))
            .push(Router::with_path("json").post(json_handler)),
    )
}

/// Salvo's 64 KiB default `secure_max_size` counts as unset: the shared
/// 2 MiB default applies.
#[tokio::test]
async fn default_limit_is_2_mib() {
    let big = format!(r#"{{"name":"{}"}}"#, "x".repeat(128 * 1024));
    let resp = TestClient::post("http://localhost/json")
        .add_header("content-type", "application/json", true)
        .body(big)
        .send(&service())
        .await;
    assert_eq!(resp.status_code.unwrap(), StatusCode::OK);
}

#[tokio::test]
async fn urlencoded_form_is_accepted() {
    let mut resp = TestClient::post("http://localhost/form")
        .add_header("content-type", "application/x-www-form-urlencoded", true)
        .body("username=alice+b&age=30&tags=a&tags=b")
        .send(&service())
        .await;
    assert_eq!(resp.status_code.unwrap(), StatusCode::OK);
    assert_eq!(resp.take_string().await.unwrap(), "alice b:30:a,b");
}

#[tokio::test]
async fn form_validation_error_is_422() {
    let mut resp = TestClient::post("http://localhost/form")
        .add_header("content-type", "application/x-www-form-urlencoded", true)
        .body("username=al&age=-1")
        .send(&service())
        .await;
    assert_eq!(resp.status_code.unwrap(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = resp.take_json().await.unwrap();
    assert_eq!(body["error"], "Validation failed");
}

#[tokio::test]
async fn form_too_large_is_413_json() {
    let big = format!("username={}", "x".repeat(128 * 1024));
    let mut resp = TestClient::post("http://localhost/form")
        .add_header("content-type", "application/x-www-form-urlencoded", true)
        .body(big)
        .send(&limited_service())
        .await;
    assert_eq!(resp.status_code.unwrap(), StatusCode::PAYLOAD_TOO_LARGE);
    let body: serde_json::Value = resp.take_json().await.unwrap();
    assert_eq!(body["error"], "Payload too large");
}

#[tokio::test]
async fn json_too_large_is_413_json() {
    let big = format!(r#"{{"name":"{}"}}"#, "x".repeat(128 * 1024));
    let mut resp = TestClient::post("http://localhost/json")
        .add_header("content-type", "application/json", true)
        .body(big)
        .send(&limited_service())
        .await;
    assert_eq!(resp.status_code.unwrap(), StatusCode::PAYLOAD_TOO_LARGE);
    let body: serde_json::Value = resp.take_json().await.unwrap();
    assert_eq!(body["error"], "Payload too large");
}
