//! Rocket example showcasing vld extractors with proper response schemas.
//!
//! Run:
//! ```sh
//! cargo run -p vld-rocket --example rocket_basic
//! ```

use serde::Serialize;
use vld_rocket::prelude::*;

// ===========================================================================
// POST /users — VldJson (JSON body)
// ===========================================================================

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CreateUserRequest {
        pub name: String  => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
        pub age: i64      => vld::number().int().min(0).max(150),
    }
}

vld::schema! {
    #[derive(Debug, Clone, Serialize)]
    pub struct CreateUserResponse {
        pub status: String => vld::string(),
        pub name: String   => vld::string(),
        pub email: String  => vld::string(),
        pub age: i64       => vld::number().int(),
    }
}

#[rocket::post("/users", data = "<user>")]
fn create_user(user: VldJson<CreateUserRequest>) -> rocket::serde::json::Json<CreateUserResponse> {
    println!("-> POST /users  {:?}", *user);
    rocket::serde::json::Json(CreateUserResponse {
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
        pub q: String      => vld::string().min(1),
        pub page: i64      => vld::number().int().min(1),
        pub limit: i64     => vld::number().int().min(1).max(100),
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

#[rocket::get("/search")]
fn search(q: VldQuery<SearchRequest>) -> rocket::serde::json::Json<SearchResponse> {
    println!("-> GET /search  {:?}", *q);
    rocket::serde::json::Json(SearchResponse {
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

#[rocket::get("/health")]
fn health() -> rocket::serde::json::Json<HealthResponse> {
    rocket::serde::json::Json(HealthResponse {
        status: "ok".into(),
    })
}

// ===========================================================================
// Main
// ===========================================================================

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    println!("=== vld-rocket example ===");
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
        r#"  curl -s -X POST http://localhost:8000/users \
    -H "Content-Type: application/json" \
    -d '{{"name":"Alice","email":"alice@example.com","age":30}}' | jq"#
    );
    println!();
    println!("  # Validation error:");
    println!(
        r#"  curl -s -X POST http://localhost:8000/users \
    -H "Content-Type: application/json" \
    -d '{{"name":"A","email":"bad","age":-1}}' | jq"#
    );
    println!();
    println!("  # Search:");
    println!(r#"  curl -s "http://localhost:8000/search?q=hello&page=1&limit=10" | jq"#);
    println!();
    println!("  # Health:");
    println!(r#"  curl -s http://localhost:8000/health | jq"#);
    println!();

    let _rocket = rocket::build()
        .mount("/", rocket::routes![create_user, search, health])
        .register(
            "/",
            rocket::catchers![vld_rocket::vld_422_catcher, vld_rocket::vld_400_catcher],
        )
        .launch()
        .await?;
    Ok(())
}
