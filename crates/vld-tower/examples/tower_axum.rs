//! `ValidateJsonLayer` on an Axum router.
//!
//! Run:
//! ```sh
//! cargo run -p vld-tower --example tower_axum
//! curl -X POST localhost:3000/users -H 'content-type: application/json' \
//!     -d '{"name":"Alice","email":"alice@example.com"}'
//! ```

use axum::http::StatusCode;
use axum::routing::post;
use axum::{Extension, Router};
use vld_tower::{ValidateJsonLayer, Validated};

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CreateUser {
        pub name: String  => vld::string().min(2).max(100),
        pub email: String => vld::string().email(),
    }
}

/// Invalid JSON never gets here (the layer answers 422/413). A non-JSON
/// request passes the layer unvalidated, hence the `Option`.
async fn create(
    user: Option<Extension<Validated<CreateUser>>>,
) -> Result<String, (StatusCode, &'static str)> {
    let Some(Extension(Validated(user))) = user else {
        return Err((StatusCode::UNSUPPORTED_MEDIA_TYPE, "expected JSON"));
    };
    Ok(format!("created {} <{}>", user.name, user.email))
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/users", post(create))
        .layer(ValidateJsonLayer::<CreateUser>::new());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("listening on http://127.0.0.1:3000");
    axum::serve(listener, app).await.unwrap();
}
