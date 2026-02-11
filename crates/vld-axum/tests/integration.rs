use axum::body::Body;
use axum::routing::{get, post};
use axum::Router;
use http::{Request, StatusCode};
use tower::ServiceExt;
use vld::prelude::*;
use vld_axum::{VldCookie, VldForm, VldHeaders, VldJson, VldPath, VldQuery};

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

async fn json_handler(VldJson(user): VldJson<TestUser>) -> String {
    format!("{}:{}", user.name, user.age)
}

fn json_app() -> Router {
    Router::new().route("/test", post(json_handler))
}

#[tokio::test]
async fn valid_request() {
    let body = serde_json::json!({"name": "Alice", "age": 25});
    let resp = json_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let text = body_text(resp).await;
    assert_eq!(text, "Alice:25");
}

#[tokio::test]
async fn validation_error_returns_422() {
    let body = serde_json::json!({"name": "A", "age": -1});
    let resp = json_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = body_json(resp).await;
    assert_eq!(json["error"], "Validation failed");
    assert!(json["issues"].as_array().unwrap().len() >= 2);
}

#[tokio::test]
async fn invalid_json_returns_422() {
    let resp = json_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("content-type", "application/json")
                .body(Body::from("not json"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn missing_fields_returns_422() {
    let body = serde_json::json!({});
    let resp = json_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn wrong_type_returns_422() {
    let body = serde_json::json!([1, 2, 3]);
    let resp = json_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn rejection_display_and_debug() {
    let body = serde_json::json!({"name": "A", "age": -1});
    let resp = json_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(resp).await;
    assert!(json.get("error").is_some());
    assert!(json.get("issues").is_some());
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

async fn search_handler(VldQuery(p): VldQuery<SearchParams>) -> String {
    format!("q={} page={:?} limit={:?}", p.q, p.page, p.limit)
}

fn query_app() -> Router {
    Router::new().route("/search", get(search_handler))
}

#[tokio::test]
async fn query_valid_all_params() {
    let resp = query_app()
        .oneshot(
            Request::builder()
                .uri("/search?q=rust&page=2&limit=25")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_text(resp).await, "q=rust page=Some(2) limit=Some(25)");
}

#[tokio::test]
async fn query_valid_optional_missing() {
    let resp = query_app()
        .oneshot(
            Request::builder()
                .uri("/search?q=hello")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_text(resp).await, "q=hello page=None limit=None");
}

#[tokio::test]
async fn query_missing_required_param() {
    let resp = query_app()
        .oneshot(
            Request::builder()
                .uri("/search?page=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn query_limit_too_large() {
    let resp = query_app()
        .oneshot(
            Request::builder()
                .uri("/search?q=test&limit=500")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn query_empty_string() {
    let resp = query_app()
        .oneshot(
            Request::builder()
                .uri("/search")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn query_url_encoded_values() {
    let resp = query_app()
        .oneshot(
            Request::builder()
                .uri("/search?q=hello+world&page=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(body_text(resp).await.contains("q=hello world"));
}

#[tokio::test]
async fn query_percent_encoded() {
    let resp = query_app()
        .oneshot(
            Request::builder()
                .uri("/search?q=caf%C3%A9")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

vld::schema! {
    #[derive(Debug)]
    pub struct FilterParams {
        pub active: Option<bool> => vld::boolean().optional(),
        pub count: Option<i64> => vld::number().int().optional(),
    }
}

async fn filter_handler(VldQuery(p): VldQuery<FilterParams>) -> String {
    format!("active={:?} count={:?}", p.active, p.count)
}

#[tokio::test]
async fn query_boolean_coercion() {
    let app = Router::new().route("/filter", get(filter_handler));
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/filter?active=true&count=42")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_text(resp).await, "active=Some(true) count=Some(42)");
}

#[tokio::test]
async fn query_boolean_false() {
    let app = Router::new().route("/filter", get(filter_handler));
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/filter?active=false")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_text(resp).await, "active=Some(false) count=None");
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

async fn path_handler(VldPath(p): VldPath<UserPath>) -> String {
    format!("id={}", p.id)
}

fn path_app() -> Router {
    Router::new().route("/users/{id}", get(path_handler))
}

#[tokio::test]
async fn path_valid() {
    let resp = path_app()
        .oneshot(
            Request::builder()
                .uri("/users/42")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_text(resp).await, "id=42");
}

#[tokio::test]
async fn path_validation_error() {
    let resp = path_app()
        .oneshot(
            Request::builder()
                .uri("/users/0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn path_not_a_number() {
    let resp = path_app()
        .oneshot(
            Request::builder()
                .uri("/users/abc")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
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

async fn form_handler(VldForm(f): VldForm<LoginForm>) -> String {
    format!("user={}", f.username)
}

fn form_app() -> Router {
    Router::new().route("/login", post(form_handler))
}

#[tokio::test]
async fn form_valid() {
    let resp = form_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("username=alice&password=secret1234"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_text(resp).await, "user=alice");
}

#[tokio::test]
async fn form_validation_error() {
    let resp = form_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("username=al&password=123"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn form_missing_fields() {
    let resp = form_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .body(Body::from(""))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
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

async fn headers_handler(VldHeaders(h): VldHeaders<AuthHeaders>) -> String {
    format!("auth={} rid={:?}", h.authorization, h.x_request_id)
}

fn headers_app() -> Router {
    Router::new().route("/protected", get(headers_handler))
}

#[tokio::test]
async fn headers_valid() {
    let resp = headers_app()
        .oneshot(
            Request::builder()
                .uri("/protected")
                .header("Authorization", "Bearer token123")
                .header("X-Request-Id", "abc-123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let text = body_text(resp).await;
    assert!(text.contains("auth=Bearer token123"));
    assert!(text.contains("abc-123"));
}

#[tokio::test]
async fn headers_optional_missing() {
    let resp = headers_app()
        .oneshot(
            Request::builder()
                .uri("/protected")
                .header("Authorization", "Bearer token123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let text = body_text(resp).await;
    assert!(text.contains("rid=None"));
}

#[tokio::test]
async fn headers_missing_required() {
    let resp = headers_app()
        .oneshot(
            Request::builder()
                .uri("/protected")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
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

async fn cookie_handler(VldCookie(c): VldCookie<SessionCookies>) -> String {
    format!("sid={} theme={:?}", c.session_id, c.theme)
}

fn cookie_app() -> Router {
    Router::new().route("/dashboard", get(cookie_handler))
}

#[tokio::test]
async fn cookie_valid() {
    let resp = cookie_app()
        .oneshot(
            Request::builder()
                .uri("/dashboard")
                .header("Cookie", "session_id=abc123; theme=dark")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let text = body_text(resp).await;
    assert!(text.contains("sid=abc123"));
    assert!(text.contains("dark"));
}

#[tokio::test]
async fn cookie_optional_missing() {
    let resp = cookie_app()
        .oneshot(
            Request::builder()
                .uri("/dashboard")
                .header("Cookie", "session_id=abc123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let text = body_text(resp).await;
    assert!(text.contains("theme=None"));
}

#[tokio::test]
async fn cookie_missing_required() {
    let resp = cookie_app()
        .oneshot(
            Request::builder()
                .uri("/dashboard")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn cookie_boolean_coercion() {
    vld::schema! {
        #[derive(Debug)]
        pub struct Prefs {
            pub dark_mode: Option<bool> => vld::boolean().optional(),
        }
    }

    async fn prefs_handler(VldCookie(c): VldCookie<Prefs>) -> String {
        format!("{:?}", c.dark_mode)
    }

    let app = Router::new().route("/prefs", get(prefs_handler));
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/prefs")
                .header("Cookie", "dark_mode=true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_text(resp).await, "Some(true)");
}

// ===========================================================================
// Helpers
// ===========================================================================

async fn body_text(resp: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

async fn body_json(resp: axum::response::Response) -> serde_json::Value {
    serde_json::from_str(&body_text(resp).await).unwrap()
}
