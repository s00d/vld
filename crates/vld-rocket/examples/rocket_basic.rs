use vld_rocket::prelude::*;

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
        pub q: String      => vld::string().min(1),
        pub page: i64      => vld::number().int().min(1),
        pub limit: i64     => vld::number().int().min(1).max(100),
    }
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

#[rocket::post("/users", data = "<user>")]
fn create_user(user: VldJson<CreateUser>) -> rocket::serde::json::Json<serde_json::Value> {
    println!("Created user: {:?}", *user);
    rocket::serde::json::Json(serde_json::json!({
        "status": "ok",
        "name": user.name,
        "email": user.email,
        "age": user.age,
    }))
}

#[rocket::get("/search")]
fn search(q: VldQuery<SearchQuery>) -> rocket::serde::json::Json<serde_json::Value> {
    println!("Search: {:?}", *q);
    rocket::serde::json::Json(serde_json::json!({
        "query": q.q,
        "page": q.page,
        "limit": q.limit,
        "results": [],
    }))
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    println!("=== vld-rocket example ===");
    println!();
    println!("Routes:");
    println!("  POST /users   — create user (JSON body)");
    println!("  GET  /search  — search (query params)");
    println!();
    println!("Example requests:");
    println!(
        r#"  curl -X POST http://localhost:8000/users -H 'Content-Type: application/json' -d '{{"name":"Alice","email":"alice@example.com","age":30}}'"#
    );
    println!(r#"  curl "http://localhost:8000/search?q=hello&page=1&limit=10""#);
    println!();

    let _rocket = rocket::build()
        .mount("/", rocket::routes![create_user, search])
        .register(
            "/",
            rocket::catchers![vld_rocket::vld_422_catcher, vld_rocket::vld_400_catcher],
        )
        .launch()
        .await?;
    Ok(())
}
