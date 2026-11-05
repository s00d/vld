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
// Mounted at /api/v1: GET users/<id> — VldPath, POST login — VldForm,
// GET me — VldHeaders + VldCookie
// ===========================================================================

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct UserPath {
        pub id: i64 => vld::number().int().min(1),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct LoginForm {
        pub username: String => vld::string().min(3),
        pub password: String => vld::string().min(8),
    }
}

vld::schema! {
    /// `Authorization` → `authorization`.
    #[derive(Debug, Clone)]
    pub struct AuthHeaders {
        pub authorization: String => vld::string().min(1),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct SessionCookies {
        pub session_id: String => vld::string().min(1),
    }
}

/// Rocket needs every `<param>` as an argument too; `VldPath` validates them
/// (works under any mount point).
#[rocket::get("/users/<id>")]
fn get_user(id: &str, p: VldPath<UserPath>) -> String {
    let _ = id;
    format!("user #{}", p.id)
}

#[rocket::post("/login", data = "<form>")]
fn login(form: VldForm<LoginForm>) -> String {
    format!("Welcome, {}!", form.username)
}

#[rocket::get("/me")]
fn me(h: VldHeaders<AuthHeaders>, c: VldCookie<SessionCookies>) -> String {
    format!("auth={} session={}", h.authorization, c.session_id)
}

// ===========================================================================
// Main
// ===========================================================================

#[rocket::main]
async fn main() -> Result<(), Box<rocket::Error>> {
    println!("=== vld-rocket example ===");
    println!();
    println!("Routes:");
    println!("  POST /users   — create user (JSON body)");
    println!("  GET  /search  — search (query params)");
    println!("  GET  /health  — health check");
    println!("  GET  /api/v1/users/<id> — path params (non-root mount)");
    println!("  POST /api/v1/login      — form body");
    println!("  GET  /api/v1/me         — headers + cookies");
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
    println!("  # Path / form / headers + cookies:");
    println!(r#"  curl -s http://localhost:8000/api/v1/users/42"#);
    println!(
        r#"  curl -s -d 'username=alice&password=secret123' http://localhost:8000/api/v1/login"#
    );
    println!(
        r#"  curl -s -H 'Authorization: Bearer t' -b 'session_id=abc' http://localhost:8000/api/v1/me"#
    );
    println!();
    println!("  # Health:");
    println!(r#"  curl -s http://localhost:8000/health | jq"#);
    println!();

    let _rocket = rocket::build()
        .mount("/", rocket::routes![create_user, search, health])
        .mount("/api/v1", rocket::routes![get_user, login, me])
        .register("/", vld_catchers())
        .launch()
        .await
        .map_err(Box::new)?;
    Ok(())
}
