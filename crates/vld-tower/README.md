[![Crates.io](https://img.shields.io/crates/v/vld-tower?style=for-the-badge)](https://crates.io/crates/vld-tower)
[![docs.rs](https://img.shields.io/docsrs/vld-tower?style=for-the-badge)](https://docs.rs/vld-tower)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=for-the-badge)](https://github.com/s00d/vld)
[![GitHub issues](https://img.shields.io/badge/github-issues-orange?style=for-the-badge)](https://github.com/s00d/vld/issues)
[![GitHub stars](https://img.shields.io/badge/github-stars-yellow?style=for-the-badge)](https://github.com/s00d/vld/stargazers)

# vld-tower

[Tower](https://docs.rs/tower) middleware for validating HTTP JSON request bodies
with [vld](https://crates.io/crates/vld), for Tower services over the `http` types — e.g. an
Axum `Router` (see [`examples/tower_axum.rs`](examples/tower_axum.rs)) or a hyper service via
`hyper-util`.

## Installation

```toml
[dependencies]
vld = "0.5"
vld-tower = "0.5"
```

## How it Works

1. Intercepts incoming HTTP requests with `Content-Type: application/json`
2. Reads the body and validates against a `vld` schema
3. **Valid** — stores the parsed struct as `Validated<T>` in request extensions, passes request to the inner service
4. **Invalid** (or a body read error) — returns `422 Unprocessable Entity` with JSON error details; inner service is never called
5. **Non-JSON** requests pass through untouched

The layer never fails itself: its error type is the inner service's, and responses are
`Response<ValidatedBody<ResBody>>` (the JSON error body or the inner service's body).

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

The inner service receives `Request<ValidatedBody<B>>` (`http_body_util::Either`): the buffered
bytes of a validated JSON request, or the untouched original body of a non-JSON request.

```rust
use vld_tower::{validated, ValidatedBody};

async fn handler(req: Request<ValidatedBody<Incoming>>) -> Result<Response<...>, ...> {
    // None if the middleware was not applied or the request was not JSON
    let Some(user) = validated::<CreateUser>(&req) else {
        return Ok(unsupported_media_type());
    };
}
```

JSON is detected case-insensitively (`application/json`, `application/*+json`). The body limit
defaults to 2 MiB; use `ValidateJsonLayer::with_limit(bytes)` to change it.

### With Axum

From [`examples/tower_axum.rs`](examples/tower_axum.rs):

```rust,ignore
use axum::http::StatusCode;
use axum::{routing::post, Extension, Router};
use vld_tower::{ValidateJsonLayer, Validated};

async fn create(
    user: Option<Extension<Validated<CreateUser>>>,
) -> Result<String, (StatusCode, &'static str)> {
    // `None`: the request was not JSON, so the layer passed it through.
    let Some(Extension(Validated(user))) = user else {
        return Err((StatusCode::UNSUPPORTED_MEDIA_TYPE, "expected JSON"));
    };
    Ok(format!("created {}", user.name))
}

let app = Router::new()
    .route("/users", post(create))
    .layer(ValidateJsonLayer::<CreateUser>::new());
```

## Errors and body limits

Every error uses the shared `vld` JSON body — 422 validation / malformed input, 413 payload too large, 415 wrong `Content-Type`; e.g. `{"error": "Validation failed", "issues": [{"path": ".name", "message": "...", "code": "too_small"}]}`. See [error bodies](https://github.com/s00d/vld/blob/main/crates/vld-http-common/README.md#error-bodies).

| Body | Default limit | Configure |
|---|---|---|
| JSON (`ValidateJsonLayer`) | 2 MiB | `ValidateJsonLayer::with_limit(n)` |

Limits of every adapter: [body limits](https://github.com/s00d/vld/blob/main/crates/vld-http-common/README.md#body-limits).

## Running the Example

```bash
cargo run -p vld-tower --example tower_basic
cargo run -p vld-tower --example tower_axum
```

## OpenAPI (vld-utoipa)

Tower middleware validates **JSON bodies** only. For OpenAPI request bodies, use
`impl_to_schema!(T)` from [vld-utoipa](../vld-utoipa/README.md). Query/path parameters require
framework extractors (e.g. `vld-axum::VldQuery`) plus `#[into_params]` on a separate struct.

## License

MIT
