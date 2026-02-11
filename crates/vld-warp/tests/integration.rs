use vld_warp::prelude::*;
use warp::Filter;

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
// Routes
// ---------------------------------------------------------------------------

fn routes() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let create = warp::post()
        .and(warp::path("users"))
        .and(vld_json::<CreateUser>())
        .map(|u: CreateUser| warp::reply::json(&serde_json::json!({"name": u.name})));

    let list = warp::get()
        .and(warp::path("items"))
        .and(vld_query::<Pagination>())
        .map(|p: Pagination| warp::reply::json(&serde_json::json!({"page": p.page})));

    create.or(list)
}

fn routes_with_recovery(
) -> impl Filter<Extract = (impl warp::Reply,), Error = std::convert::Infallible> + Clone {
    routes().recover(handle_rejection)
}

// ---------------------------------------------------------------------------
// Tests — JSON body
// ---------------------------------------------------------------------------

#[tokio::test]
async fn json_valid() {
    let resp = warp::test::request()
        .method("POST")
        .path("/users")
        .header("content-type", "application/json")
        .body(r#"{"name":"Alice","email":"alice@example.com"}"#)
        .reply(&routes_with_recovery())
        .await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = serde_json::from_slice(resp.body()).unwrap();
    assert_eq!(body["name"], "Alice");
}

#[tokio::test]
async fn json_invalid() {
    let resp = warp::test::request()
        .method("POST")
        .path("/users")
        .header("content-type", "application/json")
        .body(r#"{"name":"A","email":"bad"}"#)
        .reply(&routes_with_recovery())
        .await;
    assert_eq!(resp.status(), 422);
    let body: serde_json::Value = serde_json::from_slice(resp.body()).unwrap();
    assert_eq!(body["error"], "Validation failed");
    assert!(body["issues"].as_array().unwrap().len() >= 2);
}

#[tokio::test]
async fn json_malformed() {
    let resp = warp::test::request()
        .method("POST")
        .path("/users")
        .header("content-type", "application/json")
        .body("not json at all")
        .reply(&routes_with_recovery())
        .await;
    assert_eq!(resp.status(), 400);
}

// ---------------------------------------------------------------------------
// Tests — Query
// ---------------------------------------------------------------------------

#[tokio::test]
async fn query_valid() {
    let resp = warp::test::request()
        .method("GET")
        .path("/items?page=3&limit=10")
        .reply(&routes_with_recovery())
        .await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = serde_json::from_slice(resp.body()).unwrap();
    assert_eq!(body["page"], 3);
}

#[tokio::test]
async fn query_invalid() {
    let resp = warp::test::request()
        .method("GET")
        .path("/items?page=0&limit=999")
        .reply(&routes_with_recovery())
        .await;
    assert_eq!(resp.status(), 422);
}
