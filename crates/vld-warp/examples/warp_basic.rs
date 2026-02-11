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
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    println!("=== vld-warp example ===");
    println!();
    println!("Routes:");
    println!("  POST /users   — create user (JSON body)");
    println!("  GET  /search  — search (query params)");
    println!();
    println!("Example requests:");
    println!(
        r#"  curl -X POST http://localhost:3030/users -H 'Content-Type: application/json' -d '{{"name":"Alice","email":"alice@example.com","age":30}}'"#
    );
    println!(r#"  curl "http://localhost:3030/search?q=hello&page=1&limit=10""#);
    println!();

    let create_user = warp::post()
        .and(warp::path("users"))
        .and(vld_json::<CreateUser>())
        .map(|u: CreateUser| {
            println!("Created user: {:?}", u);
            warp::reply::json(&serde_json::json!({
                "status": "ok",
                "name": u.name,
                "email": u.email,
                "age": u.age,
            }))
        });

    let search = warp::get()
        .and(warp::path("search"))
        .and(vld_query::<SearchQuery>())
        .map(|q: SearchQuery| {
            println!("Search: {:?}", q);
            warp::reply::json(&serde_json::json!({
                "query": q.q,
                "page": q.page,
                "limit": q.limit,
                "results": [],
            }))
        });

    let routes = create_user.or(search).recover(handle_rejection);

    warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
}
