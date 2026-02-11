use poem::{handler, listener::TcpListener, post, Route, Server};
use vld_poem::prelude::*;

// ---------------------------------------------------------------------------
// Schemas
// ---------------------------------------------------------------------------

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CreateUser {
        pub name: String  => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
        pub age: i64      => vld::number().int().min(0).max(150),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct SearchQuery {
        pub q: String  => vld::string().min(1),
        pub page: i64  => vld::number().int().min(1),
        pub limit: i64 => vld::number().int().min(1).max(100),
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

#[handler]
async fn create_user(user: VldJson<CreateUser>) -> poem::web::Json<serde_json::Value> {
    println!("Created user: {:?}", *user);
    poem::web::Json(serde_json::json!({
        "status": "ok",
        "name": user.name,
        "email": user.email,
        "age": user.age,
    }))
}

#[handler]
async fn search(q: VldQuery<SearchQuery>) -> poem::web::Json<serde_json::Value> {
    println!("Search: {:?}", *q);
    poem::web::Json(serde_json::json!({
        "query": q.q,
        "page": q.page,
        "limit": q.limit,
        "results": [],
    }))
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    println!("=== vld-poem example ===");
    println!();
    println!("Routes:");
    println!("  POST /users   — create user (JSON body)");
    println!("  GET  /search  — search (query params)");
    println!();
    println!("Example requests:");
    println!(
        r#"  curl -X POST http://localhost:3000/users -H 'Content-Type: application/json' -d '{{"name":"Alice","email":"alice@example.com","age":30}}'"#
    );
    println!(r#"  curl "http://localhost:3000/search?q=hello&page=1&limit=10""#);
    println!();

    let app = Route::new()
        .at("/users", post(create_user))
        .at("/search", poem::get(search));

    Server::new(TcpListener::bind("0.0.0.0:3000"))
        .run(app)
        .await
}
