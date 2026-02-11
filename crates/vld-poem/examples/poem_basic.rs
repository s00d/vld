//! Poem example showcasing vld extractors with proper response schemas.
//!
//! Run:
//! ```sh
//! cargo run -p vld-poem --example poem_basic
//! ```

use poem::{handler, listener::TcpListener, post, Route, Server};
use serde::Serialize;
use vld_poem::prelude::*;

// ===========================================================================
// POST /users — VldJson (JSON body)
// ===========================================================================

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CreateUserRequest {
        pub name: String  => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
        pub age: Option<i64> => vld::number().int().min(0).max(150).optional(),
    }
}

vld::schema! {
    #[derive(Debug, Clone, Serialize)]
    pub struct CreateUserResponse {
        pub status: String => vld::string(),
        pub name: String   => vld::string(),
        pub email: String  => vld::string(),
        pub age: Option<i64> => vld::number().int().optional(),
    }
}

#[handler]
async fn create_user(user: VldJson<CreateUserRequest>) -> poem::web::Json<CreateUserResponse> {
    println!("-> POST /users  {:?}", *user);
    poem::web::Json(CreateUserResponse {
        status: "created".into(),
        name: user.name.clone(),
        email: user.email.clone(),
        age: user.age,
    })
}

// ===========================================================================
// GET /search — VldQuery (query params)
// ===========================================================================

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct SearchRequest {
        pub q: String  => vld::string().min(1),
        pub page: i64  => vld::number().int().min(1),
        pub limit: i64 => vld::number().int().min(1).max(100),
    }
}

vld::schema! {
    #[derive(Debug, Clone, Serialize)]
    pub struct SearchResponse {
        pub query: String  => vld::string(),
        pub page: i64      => vld::number().int(),
        pub limit: i64     => vld::number().int(),
        pub total: i64     => vld::number().int(),
    }
}

#[handler]
async fn search(q: VldQuery<SearchRequest>) -> poem::web::Json<SearchResponse> {
    println!("-> GET /search  {:?}", *q);
    poem::web::Json(SearchResponse {
        query: q.q.clone(),
        page: q.page,
        limit: q.limit,
        total: 0,
    })
}

// ===========================================================================
// GET /health
// ===========================================================================

vld::schema! {
    #[derive(Debug, Clone, Serialize)]
    pub struct HealthResponse {
        pub status: String => vld::string(),
    }
}

#[handler]
async fn health() -> poem::web::Json<HealthResponse> {
    poem::web::Json(HealthResponse {
        status: "ok".into(),
    })
}

// ===========================================================================
// Main
// ===========================================================================

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    println!("=== vld-poem example ===");
    println!();
    println!("Routes:");
    println!("  POST /users   — create user (JSON body)");
    println!("  GET  /search  — search (query params)");
    println!("  GET  /health  — health check");
    println!();
    println!("Example requests:");
    println!();
    println!("  # Create user:");
    println!(
        r#"  curl -s -X POST http://localhost:3000/users \
    -H "Content-Type: application/json" \
    -d '{{"name":"Alice","email":"alice@example.com","age":30}}' | jq"#
    );
    println!();
    println!("  # Validation error:");
    println!(
        r#"  curl -s -X POST http://localhost:3000/users \
    -H "Content-Type: application/json" \
    -d '{{"name":"A","email":"bad"}}' | jq"#
    );
    println!();
    println!("  # Search:");
    println!(r#"  curl -s "http://localhost:3000/search?q=hello&page=1&limit=10" | jq"#);
    println!();
    println!("  # Health:");
    println!(r#"  curl -s http://localhost:3000/health | jq"#);
    println!();

    let app = Route::new()
        .at("/users", post(create_user))
        .at("/search", poem::get(search))
        .at("/health", poem::get(health));

    Server::new(TcpListener::bind("0.0.0.0:3000"))
        .run(app)
        .await
}
