//! Salvo example showcasing vld extractors as handler parameters.
//!
//! Run:
//! ```sh
//! cargo run -p vld-salvo --example salvo_basic
//! ```

use salvo::prelude::*;
use serde::Serialize;
use vld_salvo::prelude::*;

// ===========================================================================
// POST /users — JSON body (VldJson<T> as parameter)
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
        pub status: String   => vld::string(),
        pub name: String     => vld::string(),
        pub email: String    => vld::string(),
        pub age: Option<i64> => vld::number().int().optional(),
    }
}

#[handler]
async fn create_user(body: VldJson<CreateUserRequest>, res: &mut Response) {
    println!("-> POST /users  {:?}", body.0);
    res.render(Json(CreateUserResponse {
        status: "created".into(),
        name: body.name.clone(),
        email: body.email.clone(),
        age: body.age,
    }));
}

// ===========================================================================
// GET /users/{id} — path param (VldPath<T> as parameter)
// ===========================================================================

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct UserIdPath {
        pub id: i64 => vld::number().int().min(1),
    }
}

vld::schema! {
    #[derive(Debug, Clone, Serialize)]
    pub struct UserResponse {
        pub id: i64        => vld::number().int(),
        pub name: String   => vld::string(),
        pub email: String  => vld::string(),
    }
}

#[handler]
async fn get_user(p: VldPath<UserIdPath>, res: &mut Response) {
    println!("-> GET /users/{}", p.id);
    res.render(Json(UserResponse {
        id: p.id,
        name: "Alice".into(),
        email: "alice@example.com".into(),
    }));
}

// ===========================================================================
// GET /search — query params (VldQuery<T> as parameter)
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
        pub query: String => vld::string(),
        pub page: i64     => vld::number().int(),
        pub limit: i64    => vld::number().int(),
        pub total: i64    => vld::number().int(),
    }
}

#[handler]
async fn search(q: VldQuery<SearchRequest>, res: &mut Response) {
    println!("-> GET /search  {:?}", q.0);
    res.render(Json(SearchResponse {
        query: q.q.clone(),
        page: q.page,
        limit: q.limit,
        total: 0,
    }));
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
async fn health(res: &mut Response) {
    res.render(Json(HealthResponse {
        status: "ok".into(),
    }));
}

// ===========================================================================
// Main
// ===========================================================================

#[tokio::main]
async fn main() {
    println!("=== vld-salvo example ===");
    println!();
    println!("Routes:");
    println!("  POST /users        — create user (JSON body)");
    println!("  GET  /users/{{id}}   — get user by id (path param)");
    println!("  GET  /search       — search (query params)");
    println!("  GET  /health       — health check");
    println!();
    println!("Example requests:");
    println!();
    println!("  # Create user:");
    println!(
        r#"  curl -s -X POST http://localhost:5800/users \
    -H "Content-Type: application/json" \
    -d '{{"name":"Alice","email":"alice@example.com","age":30}}' | jq"#
    );
    println!();
    println!("  # Validation error:");
    println!(
        r#"  curl -s -X POST http://localhost:5800/users \
    -H "Content-Type: application/json" \
    -d '{{"name":"A","email":"bad"}}' | jq"#
    );
    println!();
    println!("  # Get user by id:");
    println!(r#"  curl -s http://localhost:5800/users/42 | jq"#);
    println!();
    println!("  # Search:");
    println!(r#"  curl -s "http://localhost:5800/search?q=hello&page=1&limit=10" | jq"#);
    println!();
    println!("  # Health:");
    println!(r#"  curl -s http://localhost:5800/health | jq"#);
    println!();

    let router = Router::new()
        .push(Router::with_path("users").post(create_user))
        .push(Router::with_path("users/{id}").get(get_user))
        .push(Router::with_path("search").get(search))
        .push(Router::with_path("health").get(health));

    let acceptor = TcpListener::new("0.0.0.0:5800").bind().await;
    Server::new(acceptor).serve(router).await;
}
