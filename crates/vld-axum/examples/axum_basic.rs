//! Axum example showcasing all 6 vld extractors.
//!
//! Run:
//! ```sh
//! cargo run -p vld-axum --example axum_basic
//! ```

use axum::response::Json;
use axum::{routing, Router};
use serde::Serialize;
use vld::prelude::*;
use vld_axum::{VldCookie, VldForm, VldHeaders, VldJson, VldPath, VldQuery};

// ===========================================================================
// POST /users — VldJson (JSON body)
// ===========================================================================

vld::schema! {
    #[derive(Debug)]
    pub struct CreateUserRequest {
        pub name: String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
        pub age: Option<i64> => vld::number().int().min(0).max(150).optional(),
    }
}

vld::schema! {
    #[derive(Debug, Serialize)]
    pub struct CreateUserResponse {
        pub status: String => vld::string(),
        pub name: String => vld::string(),
        pub email: String => vld::string(),
        pub age: Option<i64> => vld::number().int().optional(),
    }
}

async fn create_user(VldJson(req): VldJson<CreateUserRequest>) -> Json<CreateUserResponse> {
    println!("-> POST /users  {req:?}");
    Json(CreateUserResponse {
        status: "created".into(),
        name: req.name,
        email: req.email,
        age: req.age,
    })
}

// ===========================================================================
// GET /search — VldQuery (query params)
// ===========================================================================

vld::schema! {
    #[derive(Debug)]
    pub struct SearchRequest {
        pub q: String => vld::string().min(1).max(200),
        pub page: Option<i64> => vld::number().int().min(1).optional(),
        pub limit: Option<i64> => vld::number().int().min(1).max(100).optional(),
    }
}

vld::schema! {
    #[derive(Debug, Serialize)]
    pub struct SearchResponse {
        pub query: String => vld::string(),
        pub page: i64 => vld::number().int(),
        pub limit: i64 => vld::number().int(),
        pub total: i64 => vld::number().int(),
    }
}

async fn search(VldQuery(req): VldQuery<SearchRequest>) -> Json<SearchResponse> {
    println!("-> GET /search  {req:?}");
    Json(SearchResponse {
        query: req.q,
        page: req.page.unwrap_or(1),
        limit: req.limit.unwrap_or(20),
        total: 0,
    })
}

// ===========================================================================
// GET /users/{id} — VldPath (path params)
// ===========================================================================

vld::schema! {
    #[derive(Debug)]
    pub struct UserPath {
        pub id: i64 => vld::number().int().min(1),
    }
}

vld::schema! {
    #[derive(Debug, Serialize)]
    pub struct UserResponse {
        pub id: i64 => vld::number().int(),
        pub name: String => vld::string(),
    }
}

async fn get_user(VldPath(path): VldPath<UserPath>) -> Json<UserResponse> {
    println!("-> GET /users/:id  {path:?}");
    Json(UserResponse {
        id: path.id,
        name: format!("User #{}", path.id),
    })
}

// ===========================================================================
// POST /login — VldForm (URL-encoded form body)
// ===========================================================================

vld::schema! {
    #[derive(Debug)]
    pub struct LoginForm {
        pub username: String => vld::string().min(3).max(50),
        pub password: String => vld::string().min(8),
    }
}

vld::schema! {
    #[derive(Debug, Serialize)]
    pub struct LoginResponse {
        pub status: String => vld::string(),
        pub username: String => vld::string(),
    }
}

async fn login(VldForm(form): VldForm<LoginForm>) -> Json<LoginResponse> {
    println!("-> POST /login  {form:?}");
    Json(LoginResponse {
        status: "ok".into(),
        username: form.username,
    })
}

// ===========================================================================
// GET /protected — VldHeaders (HTTP headers)
// ===========================================================================

vld::schema! {
    #[derive(Debug)]
    pub struct AuthHeaders {
        pub authorization: String => vld::string().min(1),
        pub x_request_id: Option<String> => vld::string().optional(),
    }
}

vld::schema! {
    #[derive(Debug, Serialize)]
    pub struct ProtectedResponse {
        pub message: String => vld::string(),
        pub request_id: Option<String> => vld::string().optional(),
    }
}

async fn protected(VldHeaders(h): VldHeaders<AuthHeaders>) -> Json<ProtectedResponse> {
    println!("-> GET /protected  {h:?}");
    Json(ProtectedResponse {
        message: "Access granted".into(),
        request_id: h.x_request_id,
    })
}

// ===========================================================================
// GET /dashboard — VldCookie (cookies)
// ===========================================================================

vld::schema! {
    #[derive(Debug)]
    pub struct SessionCookies {
        pub session_id: String => vld::string().min(1),
        pub theme: Option<String> => vld::string().optional(),
    }
}

vld::schema! {
    #[derive(Debug, Serialize)]
    pub struct DashboardResponse {
        pub session_id: String => vld::string(),
        pub theme: String => vld::string(),
    }
}

async fn dashboard(VldCookie(c): VldCookie<SessionCookies>) -> Json<DashboardResponse> {
    println!("-> GET /dashboard  {c:?}");
    Json(DashboardResponse {
        session_id: c.session_id,
        theme: c.theme.unwrap_or_else(|| "light".into()),
    })
}

// ===========================================================================
// POST /orders — VldQuery + VldJson combined
// ===========================================================================

vld::schema! {
    #[derive(Debug)]
    pub struct OrderQuery {
        pub dry_run: Option<bool> => vld::boolean().optional(),
        pub currency: Option<String> => vld::string().min(3).max(3).optional(),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct OrderBody {
        pub product_id: i64 => vld::number().int().min(1),
        pub quantity: i64 => vld::number().int().min(1).max(1000),
    }
}

vld::schema! {
    #[derive(Debug, Serialize)]
    pub struct OrderResponse {
        pub status: String => vld::string(),
        pub product_id: i64 => vld::number().int(),
        pub quantity: i64 => vld::number().int(),
        pub currency: String => vld::string(),
        pub dry_run: bool => vld::boolean(),
    }
}

async fn create_order(
    VldQuery(query): VldQuery<OrderQuery>,
    VldJson(body): VldJson<OrderBody>,
) -> Json<OrderResponse> {
    println!("-> POST /orders  query={query:?} body={body:?}");
    Json(OrderResponse {
        status: if query.dry_run.unwrap_or(false) {
            "dry_run"
        } else {
            "created"
        }
        .into(),
        product_id: body.product_id,
        quantity: body.quantity,
        currency: query.currency.unwrap_or_else(|| "USD".into()),
        dry_run: query.dry_run.unwrap_or(false),
    })
}

// ===========================================================================
// GET /health
// ===========================================================================

vld::schema! {
    #[derive(Debug, Serialize)]
    pub struct HealthResponse {
        pub status: String => vld::string(),
    }
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
    })
}

// ===========================================================================

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/users", routing::post(create_user))
        .route("/users/{id}", routing::get(get_user))
        .route("/search", routing::get(search))
        .route("/login", routing::post(login))
        .route("/protected", routing::get(protected))
        .route("/dashboard", routing::get(dashboard))
        .route("/orders", routing::post(create_order))
        .route("/health", routing::get(health));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    println!("Listening on http://127.0.0.1:3000");
    println!();
    println!("Available routes:");
    println!("  POST /users          — VldJson      (JSON body)");
    println!("  GET  /users/{{id}}     — VldPath      (path params)");
    println!("  GET  /search         — VldQuery     (query params)");
    println!("  POST /login          — VldForm      (URL-encoded form)");
    println!("  GET  /protected      — VldHeaders   (HTTP headers)");
    println!("  GET  /dashboard      — VldCookie    (cookies)");
    println!("  POST /orders         — VldQuery + VldJson (combined)");
    println!("  GET  /health         — health check");
    println!();
    println!("Example requests:");
    println!();
    println!("  # VldJson — create user:");
    println!(
        r#"  curl -s -X POST http://localhost:3000/users \
    -H "Content-Type: application/json" \
    -d '{{"name": "Alice", "email": "alice@example.com", "age": 25}}' | jq"#
    );
    println!();
    println!("  # VldPath — get user by id:");
    println!(r#"  curl -s http://localhost:3000/users/42 | jq"#);
    println!();
    println!("  # VldPath — validation error (id < 1):");
    println!(r#"  curl -s http://localhost:3000/users/0 | jq"#);
    println!();
    println!("  # VldQuery — search:");
    println!(r#"  curl -s "http://localhost:3000/search?q=rust&page=1&limit=20" | jq"#);
    println!();
    println!("  # VldForm — login (URL-encoded):");
    println!(
        r#"  curl -s -X POST http://localhost:3000/login \
    -d "username=alice&password=secret1234" | jq"#
    );
    println!();
    println!("  # VldForm — validation error (password too short):");
    println!(
        r#"  curl -s -X POST http://localhost:3000/login \
    -d "username=al&password=123" | jq"#
    );
    println!();
    println!("  # VldHeaders — protected endpoint:");
    println!(
        r#"  curl -s http://localhost:3000/protected \
    -H "Authorization: Bearer mytoken123" \
    -H "X-Request-Id: abc-123" | jq"#
    );
    println!();
    println!("  # VldHeaders — missing authorization:");
    println!(r#"  curl -s http://localhost:3000/protected | jq"#);
    println!();
    println!("  # VldCookie — dashboard with cookies:");
    println!(
        r#"  curl -s http://localhost:3000/dashboard \
    -b "session_id=s3ss10n; theme=dark" | jq"#
    );
    println!();
    println!("  # VldCookie — missing session_id:");
    println!(r#"  curl -s http://localhost:3000/dashboard | jq"#);
    println!();
    println!("  # Combined — query + body:");
    println!(
        r#"  curl -s -X POST "http://localhost:3000/orders?dry_run=true&currency=USD" \
    -H "Content-Type: application/json" \
    -d '{{"product_id": 42, "quantity": 3}}' | jq"#
    );

    axum::serve(listener, app).await.unwrap();
}
