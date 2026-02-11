use poem::test::TestClient;
use poem::{handler, post, Route};
use vld_poem::prelude::*;

// ---------------------------------------------------------------------------
// Schemas
// ---------------------------------------------------------------------------

vld::schema! {
    #[derive(Debug, Clone)]
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

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

#[handler]
async fn create_user(user: VldJson<CreateUser>) -> poem::web::Json<serde_json::Value> {
    poem::web::Json(serde_json::json!({
        "name": user.name,
        "email": user.email,
    }))
}

#[handler]
async fn list_items(q: VldQuery<Pagination>) -> poem::web::Json<serde_json::Value> {
    poem::web::Json(serde_json::json!({
        "page": q.page,
        "limit": q.limit,
    }))
}

fn app() -> Route {
    Route::new()
        .at("/users", post(create_user))
        .at("/items", poem::get(list_items))
}

// ---------------------------------------------------------------------------
// Tests — JSON body
// ---------------------------------------------------------------------------

#[tokio::test]
async fn json_valid() {
    let cli = TestClient::new(app());
    let resp = cli
        .post("/users")
        .content_type("application/json")
        .body(r#"{"name": "Alice", "email": "alice@example.com"}"#)
        .send()
        .await;
    resp.assert_status_is_ok();
    let body: serde_json::Value = resp.0.into_body().into_json().await.unwrap();
    assert_eq!(body["name"], "Alice");
}

#[tokio::test]
async fn json_invalid() {
    let cli = TestClient::new(app());
    let resp = cli
        .post("/users")
        .content_type("application/json")
        .body(r#"{"name": "A", "email": "bad"}"#)
        .send()
        .await;
    resp.assert_status(poem::http::StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn json_malformed() {
    let cli = TestClient::new(app());
    let resp = cli
        .post("/users")
        .content_type("application/json")
        .body("not json")
        .send()
        .await;
    let status = resp.0.status();
    assert!(
        status == poem::http::StatusCode::BAD_REQUEST
            || status == poem::http::StatusCode::UNPROCESSABLE_ENTITY
    );
}

// ---------------------------------------------------------------------------
// Tests — Query
// ---------------------------------------------------------------------------

#[tokio::test]
async fn query_valid() {
    let cli = TestClient::new(app());
    let resp = cli
        .get("/items")
        .query("page", &"2")
        .query("limit", &"25")
        .send()
        .await;
    resp.assert_status_is_ok();
    let body: serde_json::Value = resp.0.into_body().into_json().await.unwrap();
    assert_eq!(body["page"], 2);
    assert_eq!(body["limit"], 25);
}

#[tokio::test]
async fn query_invalid() {
    let cli = TestClient::new(app());
    let resp = cli
        .get("/items")
        .query("page", &"0")
        .query("limit", &"200")
        .send()
        .await;
    resp.assert_status(poem::http::StatusCode::UNPROCESSABLE_ENTITY);
}
