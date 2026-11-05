//! Regression tests for the 0.5.0 audit fixes.

mod common;
#[cfg(feature = "ntex-4")]
use common::TestRequestExt;
use ntex::web::{self, test, App, HttpResponse};
use vld_ntex::ntex;
use vld_ntex::{VldBodyLimit, VldForm, VldJson, VldQuery};

/// Registers app state (`VldBodyLimit`) on the app.
macro_rules! with_state {
    ($app:expr, $cfg:expr) => {{
        #[cfg(feature = "ntex-3")]
        let app = $app.state($cfg);
        #[cfg(feature = "ntex-4")]
        let app = $app.with_config(web::WebAppConfig::new().set_state($cfg));
        app
    }};
}

vld::schema! {
    #[derive(Debug)]
    pub struct Named {
        pub name: String => vld::string().min(2),
    }
}

async fn json_handler(body: VldJson<Named>) -> HttpResponse {
    HttpResponse::Ok().body(body.name.clone())
}

async fn form_handler(body: VldForm<Named>) -> HttpResponse {
    HttpResponse::Ok().body(body.name.clone())
}

vld::schema! {
    #[derive(Debug)]
    pub struct TagsQuery {
        pub tag: Vec<i64> => vld::array(vld::number().int()),
    }
}

async fn tags_handler(q: VldQuery<TagsQuery>) -> HttpResponse {
    HttpResponse::Ok().body(format!("{:?}", q.tag))
}

#[ntex::test]
async fn json_overflow_returns_413_json() {
    let app = test::init_service(
        with_state!(App::new(), VldBodyLimit(8)).route("/j", web::post().to(json_handler)),
    )
    .await;
    let req = test::TestRequest::post()
        .uri("/j")
        .set_json(&serde_json::json!({"name": "a-very-long-name"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 413);
    assert_eq!(
        test::read_body(resp).await,
        r#"{"error":"Payload too large"}"#
    );
}

#[ntex::test]
async fn form_overflow_returns_413_json() {
    let app = test::init_service(
        with_state!(App::new(), VldBodyLimit(8)).route("/f", web::post().to(form_handler)),
    )
    .await;
    let req = test::TestRequest::post()
        .uri("/f")
        .header("content-type", "application/x-www-form-urlencoded")
        .set_payload("name=a-very-long-name")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 413);
    assert_eq!(
        test::read_body(resp).await,
        r#"{"error":"Payload too large"}"#
    );
}

#[ntex::test]
async fn invalid_json_is_422() {
    let app = test::init_service(App::new().route("/j", web::post().to(json_handler))).await;
    let req = test::TestRequest::post()
        .uri("/j")
        .header("content-type", "application/json")
        .set_payload("{")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 422);
}

#[ntex::test]
async fn query_repeated_keys_become_array() {
    let app = test::init_service(App::new().route("/s", web::get().to(tags_handler))).await;
    let req = test::TestRequest::get().uri("/s?tag=1&tag=2").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert_eq!(test::read_body(resp).await, "[1, 2]");
}

/// Without `VldBodyLimit` the limit is 2 MiB, not ntex's 32 KiB `JsonConfig`
/// default (and ntex's content-type parsing no longer rejects
/// `application/json;charset`).
#[ntex::test]
async fn default_json_limit_is_2_mib() {
    let app = test::init_service(App::new().route("/j", web::post().to(json_handler))).await;
    let name = "x".repeat(1024 * 1024);
    let req = test::TestRequest::post()
        .uri("/j")
        .header("content-type", "application/json;charset")
        .set_payload(format!(r#"{{"name":"{name}"}}"#))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

/// With `compress`, gzip request bodies are decoded before validation
/// (the extractor reads the raw payload itself).
#[cfg(feature = "compress")]
#[ntex::test]
async fn gzip_json_body_is_decoded() {
    use std::io::Write;
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(br#"{"name":"Alice"}"#).unwrap();
    let gz = enc.finish().unwrap();
    let app = test::init_service(App::new().route("/j", web::post().to(json_handler))).await;
    let req = test::TestRequest::post()
        .uri("/j")
        .header("content-type", "application/json")
        .header("content-encoding", "gzip")
        .set_payload(gz)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert_eq!(test::read_body(resp).await, "Alice");
}
