//! Warp example showcasing vld extractors with proper response schemas.
//!
//! Run:
//! ```sh
//! cargo run -p vld-warp --example warp_basic
//! ```

use serde::Serialize;
use vld_warp::prelude::*;
use warp::Filter;

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

// ===========================================================================
// GET /health
// ===========================================================================

vld::schema! {
    #[derive(Debug, Clone, Serialize)]
    pub struct HealthResponse {
        pub status: String => vld::string(),
    }
}

// ===========================================================================
// Main
// ===========================================================================

#[tokio::main]
async fn main() {
    println!("=== vld-warp example ===");
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
        r#"  curl -s -X POST http://localhost:3030/users \
    -H "Content-Type: application/json" \
    -d '{{"name":"Alice","email":"alice@example.com","age":30}}' | jq"#
    );
    println!();
    println!("  # Validation error (name too short):");
    println!(
        r#"  curl -s -X POST http://localhost:3030/users \
    -H "Content-Type: application/json" \
    -d '{{"name":"A","email":"bad"}}' | jq"#
    );
    println!();
    println!("  # Search:");
    println!(r#"  curl -s "http://localhost:3030/search?q=hello&page=1&limit=10" | jq"#);
    println!();
    println!("  # Health check:");
    println!(r#"  curl -s http://localhost:3030/health | jq"#);
    println!();

    let create_user = warp::post()
        .and(warp::path("users"))
        .and(vld_json::<CreateUserRequest>())
        .map(|req: CreateUserRequest| {
            println!("-> POST /users  {req:?}");
            warp::reply::json(&CreateUserResponse {
                status: "created".into(),
                name: req.name,
                email: req.email,
                age: req.age,
            })
        });

    let search = warp::get()
        .and(warp::path("search"))
        .and(vld_query::<SearchRequest>())
        .map(|req: SearchRequest| {
            println!("-> GET /search  {req:?}");
            warp::reply::json(&SearchResponse {
                query: req.q,
                page: req.page,
                limit: req.limit,
                total: 0,
            })
        });

    let health = warp::get().and(warp::path("health")).map(|| {
        warp::reply::json(&HealthResponse {
            status: "ok".into(),
        })
    });

    let routes = create_user.or(search).or(health).recover(handle_rejection);

    warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
}
