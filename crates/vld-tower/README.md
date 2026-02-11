# vld-tower

Universal [Tower](https://docs.rs/tower) middleware for validating HTTP JSON
request bodies with [vld](https://crates.io/crates/vld). Works with **any**
Tower-compatible framework: Axum, Hyper, Tonic, Warp, etc.

## Installation

```toml
[dependencies]
vld = "0.1"
vld-tower = "0.1"
```

## How it Works

1. Intercepts incoming HTTP requests with `Content-Type: application/json`
2. Reads the body and validates against a `vld` schema
3. **Valid** — stores the parsed struct in request extensions, passes request to the inner service
4. **Invalid** — returns `422 Unprocessable Entity` with JSON error details; inner service is never called
5. **Non-JSON** requests pass through untouched

## Quick Start

```rust
use vld_tower::{ValidateJsonLayer, validated};
use tower::ServiceBuilder;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CreateUser {
        pub name: String  => vld::string().min(2).max(100),
        pub email: String => vld::string().email(),
    }
}

// Apply as a Tower layer
let svc = ServiceBuilder::new()
    .layer(ValidateJsonLayer::<CreateUser>::new())
    .service_fn(handler);
```

### Extracting Validated Data

```rust
use vld_tower::{validated, try_validated};

async fn handler(req: Request<impl Body>) -> Result<Response<...>, ...> {
    // Panics if middleware not applied
    let user: CreateUser = validated(&req);

    // Returns None if not available
    let user: Option<CreateUser> = try_validated(&req);
}
```

### With Axum

```rust
use axum::{Router, routing::post};
use vld_tower::ValidateJsonLayer;

let app = Router::new()
    .route("/users", post(create_user))
    .layer(ValidateJsonLayer::<CreateUser>::new());
```

## Error Responses

### Validation Error (422)

```json
{
  "error": "Validation failed",
  "issues": [
    { "path": ".name", "message": "String must be at least 2 characters" },
    { "path": ".email", "message": "Invalid email address" }
  ]
}
```

### Malformed JSON (400)

```json
{
  "error": "Invalid JSON",
  "message": "expected value at line 1 column 1"
}
```

## Running the Example

```bash
cargo run -p vld-tower --example tower_basic
```

## License

MIT
