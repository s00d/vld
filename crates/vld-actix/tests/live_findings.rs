//! Regression tests for the live end-to-end findings (0.5.0).

use actix_web::{test, web, App, HttpResponse};
use vld_actix::{VldCookie, VldJson, VldQuery};

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

async fn json_handler(b: VldJson<Named>) -> HttpResponse {
    HttpResponse::Ok().body(b.name.clone())
}

async fn query_handler(q: VldQuery<Search>) -> HttpResponse {
    HttpResponse::Ok().body(format!("{}|{}|{}|{}", q.q, q.age, q.zip, q.tags.join(",")))
}

async fn cookie_handler(c: VldCookie<Session>) -> HttpResponse {
    HttpResponse::Ok().body(c.session.clone())
}

macro_rules! app {
    () => {
        test::init_service(
            App::new()
                .route("/j", web::post().to(json_handler))
                .route("/q", web::get().to(query_handler))
                .route("/c", web::get().to(cookie_handler)),
        )
        .await
    };
}

#[actix_rt::test]
async fn json_content_types() {
    let app = app!();
    for ct in [
        "application/json",
        "application/vnd.api+json",
        "application/problem+json; charset=utf-8",
    ] {
        let req = test::TestRequest::post()
            .uri("/j")
            .insert_header(("content-type", ct))
            .set_payload(r#"{"name":"alice"}"#)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200, "{ct}");
        assert_eq!(test::read_body(resp).await, "alice");
    }
    for ct in [None, Some("text/plain")] {
        let mut req = test::TestRequest::post()
            .uri("/j")
            .set_payload(r#"{"name":"alice"}"#);
        if let Some(ct) = ct {
            req = req.insert_header(("content-type", ct));
        }
        let resp = test::call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), 415, "{ct:?}");
        let body: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
        assert_eq!(body["error"], "Unsupported Media Type");
    }
}

#[actix_rt::test]
async fn validation_issues_carry_code() {
    let app = app!();
    let req = test::TestRequest::post()
        .uri("/j")
        .set_json(serde_json::json!({"name": "a"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 422);
    let body: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(body["issues"][0]["code"], "too_small");
}

#[actix_rt::test]
async fn coercion_follows_schema() {
    let app = app!();
    let req = test::TestRequest::get()
        .uri("/q?q=123&age=30&zip=01234&tags=a")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert_eq!(test::read_body(resp).await, "123|30|01234|a");

    let req = test::TestRequest::get()
        .uri("/c")
        .insert_header(("cookie", "session=42"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert_eq!(test::read_body(resp).await, "42");
}
