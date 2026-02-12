use salvo::prelude::*;
use salvo::test::{ResponseExt, TestClient};
use serde::Serialize;
use vld_salvo::prelude::*;

// ---------------------------------------------------------------------------
// Schemas
// ---------------------------------------------------------------------------

vld::schema! {
    #[derive(Debug, Clone, Serialize)]
    pub struct CreateUser {
        pub name: String  => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Pagination {
        pub page: i64  => vld::number().int().min(1),
        pub limit: i64 => vld::number().int().min(1).max(100),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct UserId {
        pub id: i64 => vld::number().int().min(1),
    }
}

vld::schema! {
    #[derive(Debug, Clone, Serialize)]
    pub struct UserResponse {
        pub id: i64      => vld::number().int(),
        pub name: String => vld::string(),
    }
}

// ---------------------------------------------------------------------------
// Handlers — extractors as parameters (like Axum!)
// ---------------------------------------------------------------------------

#[handler]
async fn create_user_handler(body: VldJson<CreateUser>, res: &mut Response) {
    res.render(Json(
        serde_json::json!({"name": body.name, "email": body.email}),
    ));
}

#[handler]
async fn search_handler(q: VldQuery<Pagination>, res: &mut Response) {
    res.render(Json(serde_json::json!({"page": q.page, "limit": q.limit})));
}

#[handler]
async fn get_user_handler(p: VldPath<UserId>, res: &mut Response) {
    res.render(Json(UserResponse {
        id: p.id,
        name: "Alice".into(),
    }));
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

fn build_router() -> Router {
    Router::new()
        .push(Router::with_path("users").post(create_user_handler))
        .push(Router::with_path("search").get(search_handler))
        .push(Router::with_path("users/{id}").get(get_user_handler))
}

// ---------------------------------------------------------------------------
// Tests — JSON body
// ---------------------------------------------------------------------------

#[tokio::test]
async fn json_valid() {
    let service = Service::new(build_router());
    let mut resp = TestClient::post("http://localhost/users")
        .json(&serde_json::json!({"name": "Alice", "email": "alice@example.com"}))
        .send(&service)
        .await;
    assert_eq!(resp.status_code.unwrap(), StatusCode::OK);
    let body: serde_json::Value = resp.take_json().await.unwrap();
    assert_eq!(body["name"], "Alice");
}

#[tokio::test]
async fn json_invalid() {
    let service = Service::new(build_router());
    let mut resp = TestClient::post("http://localhost/users")
        .json(&serde_json::json!({"name": "A", "email": "bad"}))
        .send(&service)
        .await;
    assert_eq!(resp.status_code.unwrap(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = resp.take_json().await.unwrap();
    assert_eq!(body["error"], "Validation failed");
    assert!(!body["issues"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn json_malformed() {
    let service = Service::new(build_router());
    let resp = TestClient::post("http://localhost/users")
        .body("not json at all")
        .add_header("content-type", "application/json", true)
        .send(&service)
        .await;
    assert_eq!(resp.status_code.unwrap(), StatusCode::UNPROCESSABLE_ENTITY);
}

// ---------------------------------------------------------------------------
// Tests — Query
// ---------------------------------------------------------------------------

#[tokio::test]
async fn query_valid() {
    let service = Service::new(build_router());
    let mut resp = TestClient::get("http://localhost/search?page=3&limit=10")
        .send(&service)
        .await;
    assert_eq!(resp.status_code.unwrap(), StatusCode::OK);
    let body: serde_json::Value = resp.take_json().await.unwrap();
    assert_eq!(body["page"], 3);
    assert_eq!(body["limit"], 10);
}

#[tokio::test]
async fn query_invalid() {
    let service = Service::new(build_router());
    let resp = TestClient::get("http://localhost/search?page=0&limit=999")
        .send(&service)
        .await;
    assert_eq!(resp.status_code.unwrap(), StatusCode::UNPROCESSABLE_ENTITY);
}

// ---------------------------------------------------------------------------
// Tests — Path params
// ---------------------------------------------------------------------------

#[tokio::test]
async fn params_valid() {
    let service = Service::new(build_router());
    let mut resp = TestClient::get("http://localhost/users/42")
        .send(&service)
        .await;
    assert_eq!(resp.status_code.unwrap(), StatusCode::OK);
    let body: serde_json::Value = resp.take_json().await.unwrap();
    assert_eq!(body["id"], 42);
    assert_eq!(body["name"], "Alice");
}

#[tokio::test]
async fn params_invalid() {
    let service = Service::new(build_router());
    let mut resp = TestClient::get("http://localhost/users/0")
        .send(&service)
        .await;
    assert_eq!(resp.status_code.unwrap(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = resp.take_json().await.unwrap();
    assert_eq!(body["error"], "Validation failed");
}
