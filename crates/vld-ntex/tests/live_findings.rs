//! Regression tests for the live end-to-end findings (0.5.0).

use ntex::web::{self, test, App, HttpResponse};
use vld_ntex::ntex;
use vld_ntex::{VldJson, VldQuery};

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

async fn json_handler(b: VldJson<Named>) -> HttpResponse {
    HttpResponse::Ok().body(b.name.clone())
}

async fn query_handler(q: VldQuery<Search>) -> HttpResponse {
    HttpResponse::Ok().body(format!("{}|{}|{}|{}", q.q, q.age, q.zip, q.tags.join(",")))
}

/// ntex 4 renamed `set_payload` to `payload`.
macro_rules! payload {
    ($req:expr, $body:expr) => {{
        #[cfg(feature = "ntex-3")]
        let r = $req.set_payload($body);
        #[cfg(feature = "ntex-4")]
        let r = $req.payload($body);
        r
    }};
}

macro_rules! app {
    () => {
        test::init_service(
            App::new()
                .route("/j", web::post().to(json_handler))
                .route("/q", web::get().to(query_handler)),
        )
        .await
    };
}

#[ntex::test]
async fn json_content_types() {
    let app = app!();
    for ct in [
        "application/json",
        "application/vnd.api+json",
        "application/problem+json; charset=utf-8",
    ] {
        let req = payload!(
            test::TestRequest::post()
                .uri("/j")
                .header("content-type", ct),
            r#"{"name":"alice"}"#
        )
        .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200, "{ct}");
        assert_eq!(test::read_body(resp).await, "alice");
    }
    for ct in [None, Some("text/plain")] {
        let mut req = test::TestRequest::post().uri("/j");
        if let Some(ct) = ct {
            req = req.header("content-type", ct);
        }
        let req = payload!(req, r#"{"name":"alice"}"#).to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 415, "{ct:?}");
        let body: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
        assert_eq!(body["error"], "Unsupported Media Type");
    }
}

#[ntex::test]
async fn validation_issues_carry_code() {
    let app = app!();
    let req = payload!(
        test::TestRequest::post()
            .uri("/j")
            .header("content-type", "application/json"),
        r#"{"name":"a"}"#
    )
    .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 422);
    let body: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(body["issues"][0]["code"], "too_small");
}

#[ntex::test]
async fn query_coercion_follows_schema() {
    let app = app!();
    let req = test::TestRequest::get()
        .uri("/q?q=123&age=30&zip=01234&tags=a")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert_eq!(test::read_body(resp).await, "123|30|01234|a");
}
