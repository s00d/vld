use actix_web::{test, web, App, HttpResponse};
use vld::prelude::*;
use vld_actix::{VldCookie, VldForm, VldHeaders, VldJson, VldPath, VldQuery};

// ===========================================================================
// VldJson tests
// ===========================================================================

vld::schema! {
    #[derive(Debug)]
    pub struct TestUser {
        pub name: String => vld::string().min(2).max(50),
        pub age: i64 => vld::number().int().min(0),
    }
}

async fn json_handler(body: VldJson<TestUser>) -> HttpResponse {
    HttpResponse::Ok().body(format!("{}:{}", body.name, body.age))
}

#[actix_rt::test]
async fn json_valid_request() {
    let app = test::init_service(App::new().route("/test", web::post().to(json_handler))).await;
    let req = test::TestRequest::post()
        .uri("/test")
        .set_json(serde_json::json!({"name": "Alice", "age": 25}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert_eq!(test::read_body(resp).await, bytes::Bytes::from("Alice:25"));
}

#[actix_rt::test]
async fn json_validation_error_returns_422() {
    let app = test::init_service(App::new().route("/test", web::post().to(json_handler))).await;
    let req = test::TestRequest::post()
        .uri("/test")
        .set_json(serde_json::json!({"name": "A", "age": -1}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 422);
    let body = test::read_body(resp).await;
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "Validation failed");
    assert!(json["issues"].as_array().unwrap().len() >= 2);
}

#[actix_rt::test]
async fn json_invalid_json_returns_error() {
    let app = test::init_service(App::new().route("/test", web::post().to(json_handler))).await;
    let req = test::TestRequest::post()
        .uri("/test")
        .insert_header(("content-type", "application/json"))
        .set_payload("not json")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_client_error());
}

#[actix_rt::test]
async fn json_missing_fields_returns_422() {
    let app = test::init_service(App::new().route("/test", web::post().to(json_handler))).await;
    let req = test::TestRequest::post()
        .uri("/test")
        .set_json(serde_json::json!({}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 422);
}

#[actix_rt::test]
async fn json_wrong_type_returns_422() {
    let app = test::init_service(App::new().route("/test", web::post().to(json_handler))).await;
    let req = test::TestRequest::post()
        .uri("/test")
        .set_json(serde_json::json!([1, 2, 3]))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 422);
}

#[actix_rt::test]
async fn json_deref_works() {
    let user = TestUser {
        name: "Alice".into(),
        age: 25,
    };
    let wrapper = VldJson(user);
    assert_eq!(wrapper.name, "Alice");
    assert_eq!(wrapper.age, 25);
}

// ===========================================================================
// VldQuery tests
// ===========================================================================

vld::schema! {
    #[derive(Debug)]
    pub struct SearchParams {
        pub q: String => vld::string().min(1).max(200),
        pub page: Option<i64> => vld::number().int().min(1).optional(),
        pub limit: Option<i64> => vld::number().int().min(1).max(100).optional(),
    }
}

async fn search_handler(params: VldQuery<SearchParams>) -> HttpResponse {
    HttpResponse::Ok().body(format!(
        "q={} page={:?} limit={:?}",
        params.q, params.page, params.limit
    ))
}

#[actix_rt::test]
async fn query_valid_all_params() {
    let app = test::init_service(App::new().route("/search", web::get().to(search_handler))).await;
    let req = test::TestRequest::get()
        .uri("/search?q=rust&page=2&limit=25")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert_eq!(body, "q=rust page=Some(2) limit=Some(25)");
}

#[actix_rt::test]
async fn query_valid_optional_missing() {
    let app = test::init_service(App::new().route("/search", web::get().to(search_handler))).await;
    let req = test::TestRequest::get().uri("/search?q=hello").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert_eq!(body, "q=hello page=None limit=None");
}

#[actix_rt::test]
async fn query_missing_required_param() {
    let app = test::init_service(App::new().route("/search", web::get().to(search_handler))).await;
    let req = test::TestRequest::get().uri("/search?page=1").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 422);
}

#[actix_rt::test]
async fn query_limit_too_large() {
    let app = test::init_service(App::new().route("/search", web::get().to(search_handler))).await;
    let req = test::TestRequest::get()
        .uri("/search?q=test&limit=500")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 422);
}

#[actix_rt::test]
async fn query_empty_returns_422() {
    let app = test::init_service(App::new().route("/search", web::get().to(search_handler))).await;
    let req = test::TestRequest::get().uri("/search").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 422);
}

#[actix_rt::test]
async fn query_url_encoded_values() {
    let app = test::init_service(App::new().route("/search", web::get().to(search_handler))).await;
    let req = test::TestRequest::get()
        .uri("/search?q=hello+world&page=1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert!(body.contains("q=hello world"));
}

vld::schema! {
    #[derive(Debug)]
    pub struct FilterParams {
        pub active: Option<bool> => vld::boolean().optional(),
        pub count: Option<i64> => vld::number().int().optional(),
    }
}

async fn filter_handler(params: VldQuery<FilterParams>) -> HttpResponse {
    HttpResponse::Ok().body(format!(
        "active={:?} count={:?}",
        params.active, params.count
    ))
}

#[actix_rt::test]
async fn query_boolean_coercion() {
    let app = test::init_service(App::new().route("/filter", web::get().to(filter_handler))).await;
    let req = test::TestRequest::get()
        .uri("/filter?active=true&count=42")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert_eq!(body, "active=Some(true) count=Some(42)");
}

#[actix_rt::test]
async fn query_boolean_false() {
    let app = test::init_service(App::new().route("/filter", web::get().to(filter_handler))).await;
    let req = test::TestRequest::get()
        .uri("/filter?active=false")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert_eq!(body, "active=Some(false) count=None");
}

// ===========================================================================
// VldPath tests
// ===========================================================================

vld::schema! {
    #[derive(Debug)]
    pub struct UserPath {
        pub id: i64 => vld::number().int().min(1),
    }
}

async fn path_handler(path: VldPath<UserPath>) -> HttpResponse {
    HttpResponse::Ok().body(format!("id={}", path.id))
}

#[actix_rt::test]
async fn path_valid() {
    let app =
        test::init_service(App::new().route("/users/{id}", web::get().to(path_handler))).await;
    let req = test::TestRequest::get().uri("/users/42").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert_eq!(body, "id=42");
}

#[actix_rt::test]
async fn path_validation_error() {
    let app =
        test::init_service(App::new().route("/users/{id}", web::get().to(path_handler))).await;
    let req = test::TestRequest::get().uri("/users/0").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 422);
}

#[actix_rt::test]
async fn path_not_a_number() {
    let app =
        test::init_service(App::new().route("/users/{id}", web::get().to(path_handler))).await;
    let req = test::TestRequest::get().uri("/users/abc").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 422);
}

// ===========================================================================
// VldForm tests
// ===========================================================================

vld::schema! {
    #[derive(Debug)]
    pub struct LoginForm {
        pub username: String => vld::string().min(3).max(50),
        pub password: String => vld::string().min(8),
    }
}

async fn form_handler(form: VldForm<LoginForm>) -> HttpResponse {
    HttpResponse::Ok().body(format!("user={}", form.username))
}

#[actix_rt::test]
async fn form_valid() {
    let app = test::init_service(App::new().route("/login", web::post().to(form_handler))).await;
    let req = test::TestRequest::post()
        .uri("/login")
        .insert_header(("content-type", "application/x-www-form-urlencoded"))
        .set_payload("username=alice&password=secret1234")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert_eq!(body, "user=alice");
}

#[actix_rt::test]
async fn form_validation_error() {
    let app = test::init_service(App::new().route("/login", web::post().to(form_handler))).await;
    let req = test::TestRequest::post()
        .uri("/login")
        .insert_header(("content-type", "application/x-www-form-urlencoded"))
        .set_payload("username=al&password=123")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 422);
}

#[actix_rt::test]
async fn form_missing_fields() {
    let app = test::init_service(App::new().route("/login", web::post().to(form_handler))).await;
    let req = test::TestRequest::post()
        .uri("/login")
        .set_payload("")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 422);
}

// ===========================================================================
// VldHeaders tests
// ===========================================================================

vld::schema! {
    #[derive(Debug)]
    pub struct AuthHeaders {
        pub authorization: String => vld::string().min(1),
        pub x_request_id: Option<String> => vld::string().optional(),
    }
}

async fn headers_handler(h: VldHeaders<AuthHeaders>) -> HttpResponse {
    HttpResponse::Ok().body(format!("auth={} rid={:?}", h.authorization, h.x_request_id))
}

#[actix_rt::test]
async fn headers_valid() {
    let app =
        test::init_service(App::new().route("/protected", web::get().to(headers_handler))).await;
    let req = test::TestRequest::get()
        .uri("/protected")
        .insert_header(("Authorization", "Bearer token123"))
        .insert_header(("X-Request-Id", "abc-123"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert!(body.contains("auth=Bearer token123"));
    assert!(body.contains("abc-123"));
}

#[actix_rt::test]
async fn headers_optional_missing() {
    let app =
        test::init_service(App::new().route("/protected", web::get().to(headers_handler))).await;
    let req = test::TestRequest::get()
        .uri("/protected")
        .insert_header(("Authorization", "Bearer token123"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert!(body.contains("rid=None"));
}

#[actix_rt::test]
async fn headers_missing_required() {
    let app =
        test::init_service(App::new().route("/protected", web::get().to(headers_handler))).await;
    let req = test::TestRequest::get().uri("/protected").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 422);
}

// ===========================================================================
// VldCookie tests
// ===========================================================================

vld::schema! {
    #[derive(Debug)]
    pub struct SessionCookies {
        pub session_id: String => vld::string().min(1),
        pub theme: Option<String> => vld::string().optional(),
    }
}

async fn cookie_handler(c: VldCookie<SessionCookies>) -> HttpResponse {
    HttpResponse::Ok().body(format!("sid={} theme={:?}", c.session_id, c.theme))
}

#[actix_rt::test]
async fn cookie_valid() {
    let app =
        test::init_service(App::new().route("/dashboard", web::get().to(cookie_handler))).await;
    let req = test::TestRequest::get()
        .uri("/dashboard")
        .insert_header(("Cookie", "session_id=abc123; theme=dark"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert!(body.contains("sid=abc123"));
    assert!(body.contains("dark"));
}

#[actix_rt::test]
async fn cookie_optional_missing() {
    let app =
        test::init_service(App::new().route("/dashboard", web::get().to(cookie_handler))).await;
    let req = test::TestRequest::get()
        .uri("/dashboard")
        .insert_header(("Cookie", "session_id=abc123"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert!(body.contains("theme=None"));
}

#[actix_rt::test]
async fn cookie_missing_required() {
    let app =
        test::init_service(App::new().route("/dashboard", web::get().to(cookie_handler))).await;
    let req = test::TestRequest::get().uri("/dashboard").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 422);
}

#[actix_rt::test]
async fn query_deref_works() {
    let params = SearchParams {
        q: "test".into(),
        page: Some(1),
        limit: None,
    };
    let wrapper = VldQuery(params);
    assert_eq!(wrapper.q, "test");
    assert_eq!(wrapper.page, Some(1));
}
