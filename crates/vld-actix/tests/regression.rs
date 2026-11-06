//! Regression tests for the 0.5.0 audit fixes.

use actix_web::{App, HttpResponse, test, web};
use vld_actix::{VldCookie, VldForm, VldHeaders, VldJson, VldPath, VldQuery};

vld::schema! {
    #[derive(Debug)]
    pub struct UserPath {
        pub id: i64 => vld::number().int().min(1),
    }
}

async fn path_handler(path: VldPath<UserPath>) -> HttpResponse {
    HttpResponse::Ok().body(format!("id={}", path.id))
}

vld::schema! {
    #[derive(Debug)]
    pub struct Body {
        pub name: String => vld::string().min(2),
    }
}

async fn json_handler(body: VldJson<Body>) -> HttpResponse {
    HttpResponse::Ok().body(body.name.clone())
}

async fn form_handler(body: VldForm<Body>) -> HttpResponse {
    HttpResponse::Ok().body(body.name.clone())
}

vld::schema! {
    #[derive(Debug)]
    pub struct TagsQuery {
        pub tag: Vec<String> => vld::array(vld::string()),
        pub q: String => vld::string(),
    }
}

async fn tags_handler(q: VldQuery<TagsQuery>) -> HttpResponse {
    HttpResponse::Ok().body(format!("{}|{}", q.tag.join(","), q.q))
}

vld::schema! {
    #[derive(Debug)]
    pub struct Sid {
        pub sid: String => vld::string(),
    }
}

async fn cookie_handler(c: VldCookie<Sid>) -> HttpResponse {
    HttpResponse::Ok().body(c.sid.clone())
}

vld::schema! {
    #[derive(Debug)]
    pub struct TagHeader {
        pub x_tag: String => vld::string(),
    }
}

async fn header_handler(h: VldHeaders<TagHeader>) -> HttpResponse {
    HttpResponse::Ok().body(h.x_tag.clone())
}

async fn body_json(resp: actix_web::dev::ServiceResponse) -> serde_json::Value {
    serde_json::from_slice(&test::read_body(resp).await).unwrap()
}

#[actix_rt::test]
async fn path_with_regex_segment() {
    let app =
        test::init_service(App::new().route(r"/users/{id:\d+}", web::get().to(path_handler))).await;
    let req = test::TestRequest::get().uri("/users/42").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert_eq!(test::read_body(resp).await, "id=42");
}

#[actix_rt::test]
async fn form_too_large_returns_413_json() {
    let app = test::init_service(
        App::new()
            .app_data(vld_actix::VldBodyLimit(8))
            .route("/f", web::post().to(form_handler)),
    )
    .await;
    let req = test::TestRequest::post()
        .uri("/f")
        .insert_header(("content-type", "application/x-www-form-urlencoded"))
        .set_payload("name=alice-the-long")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 413);
    assert_eq!(body_json(resp).await["error"], "Payload too large");
}

#[actix_rt::test]
async fn json_too_large_returns_413_json() {
    let app = test::init_service(
        App::new()
            .app_data(vld_actix::VldBodyLimit(8))
            .route("/j", web::post().to(json_handler)),
    )
    .await;
    let req = test::TestRequest::post()
        .uri("/j")
        .set_json(serde_json::json!({"name": "alice-the-long"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 413);
    assert_eq!(body_json(resp).await["error"], "Payload too large");
}

#[actix_rt::test]
async fn invalid_json_returns_422_validation_shape() {
    let app = test::init_service(App::new().route("/j", web::post().to(json_handler))).await;
    let req = test::TestRequest::post()
        .uri("/j")
        .insert_header(("content-type", "application/json"))
        .set_payload("{")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 422);
    assert_eq!(body_json(resp).await["error"], "Validation failed");
}

#[actix_rt::test]
async fn query_arrays_and_non_canonical_strings() {
    let app = test::init_service(App::new().route("/s", web::get().to(tags_handler))).await;
    let req = test::TestRequest::get()
        .uri("/s?tag=a&tag=caf%C3%A9&q=01234")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert_eq!(test::read_body(resp).await, "a,café|01234");
}

#[actix_rt::test]
async fn cookie_quoted_and_encoded_value() {
    let app = test::init_service(App::new().route("/c", web::get().to(cookie_handler))).await;
    let req = test::TestRequest::get()
        .uri("/c")
        .insert_header(("cookie", "sid=\"abc%20def\""))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert_eq!(test::read_body(resp).await, "abc def");
}

#[actix_rt::test]
async fn repeated_and_non_ascii_headers() {
    let app = test::init_service(App::new().route("/h", web::get().to(header_handler))).await;
    let req = test::TestRequest::get()
        .uri("/h")
        .append_header(("x-tag", "a"))
        .append_header((
            "x-tag",
            actix_web::http::header::HeaderValue::from_bytes(b"caf\xC3\xA9").unwrap(),
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert_eq!(test::read_body(resp).await, "a, café");
}

/// The body limit is `VldBodyLimit` (default 2 MiB), not actix's
/// `JsonConfig` (whose content-type parsing rejected
/// `application/json;charset`).
#[actix_rt::test]
async fn body_limit_can_be_raised_above_default() {
    let app = test::init_service(
        App::new()
            .app_data(vld_actix::VldBodyLimit(4 * 1024 * 1024))
            .route("/j", web::post().to(json_handler)),
    )
    .await;
    let name = "x".repeat(3 * 1024 * 1024);
    let req = test::TestRequest::post()
        .uri("/j")
        .insert_header(("content-type", "application/json;charset"))
        .set_payload(format!(r#"{{"name":"{name}"}}"#))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}
