use serde::Serialize;
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

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct UserId {
        pub id: i64 => vld::number().int().min(1),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct PostPath {
        pub user_id: i64 => vld::number().int().min(1),
        pub post_id: i64 => vld::number().int().min(1),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CommentPath {
        pub user_id: i64  => vld::number().int().min(1),
        pub post_id: i64  => vld::number().int().min(1),
        pub comment_id: i64 => vld::number().int().min(1),
    }
}

vld::schema! {
    #[derive(Debug, Clone, Serialize)]
    pub struct UserResponse {
        pub id: i64    => vld::number().int(),
        pub name: String => vld::string(),
    }
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

fn routes() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let create = warp::post()
        .and(warp::path("users"))
        .and(warp::path::end())
        .and(vld_json::<CreateUser>())
        .map(|u: CreateUser| warp::reply::json(&serde_json::json!({"name": u.name})));

    let list = warp::get()
        .and(warp::path("items"))
        .and(warp::path::end())
        .and(vld_query::<Pagination>())
        .map(|p: Pagination| warp::reply::json(&serde_json::json!({"page": p.page})));

    // vld_param — single path param
    let get_user = warp::get()
        .and(warp::path("users"))
        .and(vld_param::<UserId>("id"))
        .and(warp::path::end())
        .map(|p: UserId| {
            warp::reply::json(&UserResponse {
                id: p.id,
                name: "Alice".into(),
            })
        });

    // vld_path — multi tail params
    let get_post = warp::get()
        .and(warp::path("posts"))
        .and(vld_path::<PostPath>(&["user_id", "post_id"]))
        .map(|p: PostPath| {
            warp::reply::json(&serde_json::json!({"user_id": p.user_id, "post_id": p.post_id}))
        });

    // validate_path_params — mixed static/dynamic segments
    let get_comment = warp::get()
        .and(warp::path("users"))
        .and(warp::path::param::<String>())
        .and(warp::path("posts"))
        .and(warp::path::param::<String>())
        .and(warp::path("comments"))
        .and(warp::path::param::<String>())
        .and(warp::path::end())
        .and_then(|uid: String, pid: String, cid: String| async move {
            validate_path_params::<CommentPath>(&[
                ("user_id", &uid),
                ("post_id", &pid),
                ("comment_id", &cid),
            ])
        })
        .map(|p: CommentPath| {
            warp::reply::json(&serde_json::json!({
                "user_id": p.user_id,
                "post_id": p.post_id,
                "comment_id": p.comment_id,
            }))
        });

    create.or(list).or(get_user).or(get_post).or(get_comment)
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

// ---------------------------------------------------------------------------
// Tests — vld_param (single path param)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn param_valid() {
    let resp = warp::test::request()
        .method("GET")
        .path("/users/42")
        .reply(&routes_with_recovery())
        .await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = serde_json::from_slice(resp.body()).unwrap();
    assert_eq!(body["id"], 42);
    assert_eq!(body["name"], "Alice");
}

#[tokio::test]
async fn param_invalid_zero() {
    let resp = warp::test::request()
        .method("GET")
        .path("/users/0")
        .reply(&routes_with_recovery())
        .await;
    assert_eq!(resp.status(), 422);
    let body: serde_json::Value = serde_json::from_slice(resp.body()).unwrap();
    assert_eq!(body["error"], "Validation failed");
    assert!(!body["issues"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn param_invalid_negative() {
    let resp = warp::test::request()
        .method("GET")
        .path("/users/-5")
        .reply(&routes_with_recovery())
        .await;
    assert_eq!(resp.status(), 422);
}

#[tokio::test]
async fn param_invalid_string() {
    // "abc" cannot be coerced to i64 → validation error
    let resp = warp::test::request()
        .method("GET")
        .path("/users/abc")
        .reply(&routes_with_recovery())
        .await;
    assert_eq!(resp.status(), 422);
}

// ---------------------------------------------------------------------------
// Tests — vld_path (multi tail params)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn path_multi_valid() {
    let resp = warp::test::request()
        .method("GET")
        .path("/posts/7/99")
        .reply(&routes_with_recovery())
        .await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = serde_json::from_slice(resp.body()).unwrap();
    assert_eq!(body["user_id"], 7);
    assert_eq!(body["post_id"], 99);
}

#[tokio::test]
async fn path_multi_invalid() {
    let resp = warp::test::request()
        .method("GET")
        .path("/posts/0/99")
        .reply(&routes_with_recovery())
        .await;
    assert_eq!(resp.status(), 422);
}

#[tokio::test]
async fn path_multi_wrong_segment_count() {
    // Only 1 segment instead of 2 → not found
    let resp = warp::test::request()
        .method("GET")
        .path("/posts/7")
        .reply(&routes_with_recovery())
        .await;
    assert_eq!(resp.status(), 404);
}

// ---------------------------------------------------------------------------
// Tests — validate_path_params (mixed static/dynamic)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn validate_params_valid() {
    let resp = warp::test::request()
        .method("GET")
        .path("/users/1/posts/2/comments/3")
        .reply(&routes_with_recovery())
        .await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = serde_json::from_slice(resp.body()).unwrap();
    assert_eq!(body["user_id"], 1);
    assert_eq!(body["post_id"], 2);
    assert_eq!(body["comment_id"], 3);
}

#[tokio::test]
async fn validate_params_invalid() {
    let resp = warp::test::request()
        .method("GET")
        .path("/users/0/posts/2/comments/3")
        .reply(&routes_with_recovery())
        .await;
    assert_eq!(resp.status(), 422);
    let body: serde_json::Value = serde_json::from_slice(resp.body()).unwrap();
    assert_eq!(body["error"], "Validation failed");
}
