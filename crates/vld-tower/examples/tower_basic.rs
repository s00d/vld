//! Tower middleware example with vld response schemas.
//!
//! Run:
//! ```sh
//! cargo run -p vld-tower --example tower_basic
//! ```

use bytes::Bytes;
use http::{Request, Response, StatusCode};
use http_body_util::BodyExt;
use serde::Serialize;
use tower::{ServiceBuilder, ServiceExt};
use vld_tower::{try_validated, validated, ValidateJsonLayer};

// ===========================================================================
// Schemas
// ===========================================================================

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CreateUser {
        pub name: String  => vld::string().min(2).max(100),
        pub email: String => vld::string().email(),
    }
}

vld::schema! {
    #[derive(Debug, Clone, Serialize)]
    pub struct CreateUserResponse {
        pub status: String => vld::string(),
        pub name: String   => vld::string(),
        pub email: String  => vld::string(),
    }
}

// ===========================================================================
// Handler
// ===========================================================================

async fn handler(
    req: Request<http_body_util::Full<Bytes>>,
) -> Result<Response<http_body_util::Full<Bytes>>, Box<dyn std::error::Error + Send + Sync>> {
    let user: CreateUser = validated(&req);
    println!("Validated user: {:?}", user);

    let resp = CreateUserResponse {
        status: "created".into(),
        name: user.name,
        email: user.email,
    };

    Ok(Response::builder()
        .status(StatusCode::CREATED)
        .header("content-type", "application/json")
        .body(http_body_util::Full::new(Bytes::from(
            serde_json::to_vec(&resp).unwrap(),
        )))
        .unwrap())
}

// ===========================================================================
// Main
// ===========================================================================

#[tokio::main]
async fn main() {
    let svc = ServiceBuilder::new()
        .layer(ValidateJsonLayer::<CreateUser>::new())
        .service_fn(handler);

    // --- Valid request ---
    println!("=== Valid request ===");
    let req = Request::builder()
        .method("POST")
        .uri("/users")
        .header("content-type", "application/json")
        .body(http_body_util::Full::new(Bytes::from(
            r#"{"name": "Alice", "email": "alice@example.com"}"#,
        )))
        .unwrap();

    let resp = svc.clone().oneshot(req).await.unwrap();
    println!("Status: {}", resp.status());
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    println!("Body: {}", String::from_utf8_lossy(&body));

    // --- Invalid request ---
    println!("\n=== Invalid request ===");
    let req = Request::builder()
        .method("POST")
        .uri("/users")
        .header("content-type", "application/json")
        .body(http_body_util::Full::new(Bytes::from(
            r#"{"name": "A", "email": "bad"}"#,
        )))
        .unwrap();

    let resp = svc.clone().oneshot(req).await.unwrap();
    println!("Status: {}", resp.status());
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    println!(
        "Body: {}",
        serde_json::to_string_pretty(&serde_json::from_slice::<serde_json::Value>(&body).unwrap())
            .unwrap()
    );

    // --- Non-JSON request (passes through) ---
    println!("\n=== Non-JSON request (passes through) ===");
    let svc_text = ServiceBuilder::new()
        .layer(ValidateJsonLayer::<CreateUser>::new())
        .service_fn(|req: Request<http_body_util::Full<Bytes>>| async move {
            let user: Option<CreateUser> = try_validated(&req);
            let msg = match user {
                Some(u) => format!("Got user: {:?}", u),
                None => "No validated data (non-JSON request)".to_string(),
            };
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(
                Response::builder()
                    .body(http_body_util::Full::new(Bytes::from(msg)))
                    .unwrap(),
            )
        });
    let req = Request::builder()
        .method("POST")
        .uri("/users")
        .header("content-type", "text/plain")
        .body(http_body_util::Full::new(Bytes::from("hello")))
        .unwrap();

    let resp = svc_text.oneshot(req).await.unwrap();
    println!("Status: {}", resp.status());
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    println!("Body: {}", String::from_utf8_lossy(&body));
}
