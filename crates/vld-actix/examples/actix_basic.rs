//! Actix-web example showcasing all 6 vld extractors.
//!
//! Run:
//! ```sh
//! cargo run -p vld-actix --example actix_basic
//! ```

use actix_web::{web, App, HttpResponse, HttpServer};
use serde::Serialize;
use vld::prelude::*;
use vld_actix::{VldCookie, VldForm, VldHeaders, VldJson, VldPath, VldQuery};

// ===========================================================================
// POST /users — VldJson (JSON body)
// ===========================================================================

vld::schema! {
    #[derive(Debug)]
    pub struct CreateUserRequest {
        pub name: String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}

vld::schema! {
    #[derive(Debug, Serialize)]
    pub struct CreateUserResponse {
        pub status: String => vld::string(),
        pub name: String => vld::string(),
        pub email: String => vld::string(),
    }
}

async fn create_user(body: VldJson<CreateUserRequest>) -> HttpResponse {
    println!("-> POST /users  {:?}", body.0);
    let resp = CreateUserResponse {
        status: "created".into(),
        name: body.name.clone(),
        email: body.email.clone(),
    };
    HttpResponse::Ok().json(resp)
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

async fn search(params: VldQuery<SearchRequest>) -> HttpResponse {
    println!("-> GET /search  {:?}", params.0);
    let resp = SearchResponse {
        query: params.q.clone(),
        page: params.page.unwrap_or(1),
        limit: params.limit.unwrap_or(20),
        total: 0,
    };
    HttpResponse::Ok().json(resp)
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

async fn get_user(path: VldPath<UserPath>) -> HttpResponse {
    println!("-> GET /users/:id  {:?}", path.0);
    let resp = UserResponse {
        id: path.id,
        name: format!("User #{}", path.id),
    };
    HttpResponse::Ok().json(resp)
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

async fn login(form: VldForm<LoginForm>) -> HttpResponse {
    println!("-> POST /login  {:?}", form.0);
    let resp = LoginResponse {
        status: "ok".into(),
        username: form.username.clone(),
    };
    HttpResponse::Ok().json(resp)
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

async fn protected(headers: VldHeaders<AuthHeaders>) -> HttpResponse {
    println!("-> GET /protected  {:?}", headers.0);
    let resp = ProtectedResponse {
        message: "Access granted".into(),
        request_id: headers.x_request_id.clone(),
    };
    HttpResponse::Ok().json(resp)
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

async fn dashboard(cookies: VldCookie<SessionCookies>) -> HttpResponse {
    println!("-> GET /dashboard  {:?}", cookies.0);
    let resp = DashboardResponse {
        session_id: cookies.session_id.clone(),
        theme: cookies.theme.clone().unwrap_or_else(|| "light".into()),
    };
    HttpResponse::Ok().json(resp)
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

async fn create_order(query: VldQuery<OrderQuery>, body: VldJson<OrderBody>) -> HttpResponse {
    println!("-> POST /orders  query={:?} body={:?}", query.0, body.0);
    let resp = OrderResponse {
        status: if query.dry_run.unwrap_or(false) {
            "dry_run"
        } else {
            "created"
        }
        .into(),
        product_id: body.product_id,
        quantity: body.quantity,
        currency: query.currency.clone().unwrap_or_else(|| "USD".into()),
        dry_run: query.dry_run.unwrap_or(false),
    };
    HttpResponse::Ok().json(resp)
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

async fn health() -> HttpResponse {
    HttpResponse::Ok().json(HealthResponse {
        status: "ok".into(),
    })
}

// ===========================================================================

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Listening on http://127.0.0.1:8080");
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
        r#"  curl -s -X POST http://localhost:8080/users \
    -H "Content-Type: application/json" \
    -d '{{"name": "Alice", "email": "alice@example.com"}}' | jq"#
    );
    println!();
    println!("  # VldPath — get user by id:");
    println!(r#"  curl -s http://localhost:8080/users/42 | jq"#);
    println!();
    println!("  # VldPath — validation error (id < 1):");
    println!(r#"  curl -s http://localhost:8080/users/0 | jq"#);
    println!();
    println!("  # VldQuery — search:");
    println!(r#"  curl -s "http://localhost:8080/search?q=rust&page=1&limit=20" | jq"#);
    println!();
    println!("  # VldForm — login (URL-encoded):");
    println!(
        r#"  curl -s -X POST http://localhost:8080/login \
    -d "username=alice&password=secret1234" | jq"#
    );
    println!();
    println!("  # VldForm — validation error (password too short):");
    println!(
        r#"  curl -s -X POST http://localhost:8080/login \
    -d "username=al&password=123" | jq"#
    );
    println!();
    println!("  # VldHeaders — protected endpoint:");
    println!(
        r#"  curl -s http://localhost:8080/protected \
    -H "Authorization: Bearer mytoken123" \
    -H "X-Request-Id: abc-123" | jq"#
    );
    println!();
    println!("  # VldHeaders — missing authorization:");
    println!(r#"  curl -s http://localhost:8080/protected | jq"#);
    println!();
    println!("  # VldCookie — dashboard with cookies:");
    println!(
        r#"  curl -s http://localhost:8080/dashboard \
    -b "session_id=s3ss10n; theme=dark" | jq"#
    );
    println!();
    println!("  # VldCookie — missing session_id:");
    println!(r#"  curl -s http://localhost:8080/dashboard | jq"#);
    println!();
    println!("  # Combined — query + body:");
    println!(
        r#"  curl -s -X POST "http://localhost:8080/orders?dry_run=true&currency=USD" \
    -H "Content-Type: application/json" \
    -d '{{"product_id": 42, "quantity": 3}}' | jq"#
    );

    HttpServer::new(|| {
        App::new()
            .route("/users", web::post().to(create_user))
            .route("/users/{id}", web::get().to(get_user))
            .route("/search", web::get().to(search))
            .route("/login", web::post().to(login))
            .route("/protected", web::get().to(protected))
            .route("/dashboard", web::get().to(dashboard))
            .route("/orders", web::post().to(create_order))
            .route("/health", web::get().to(health))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
