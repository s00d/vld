# vld-tower

[![crates.io](https://img.shields.io/crates/v/vld-tower.svg)](https://crates.io/crates/vld-tower)
[![docs.rs](https://img.shields.io/docsrs/vld-tower)](https://docs.rs/vld-tower)

A Tower layer that validates JSON request bodies with the [vld](https://crates.io/crates/vld) validation library. Framework-agnostic: it depends on `tower-layer`, `tower-service` and `http`, not on a web framework, so it works with Axum, hyper or a plain `service_fn`.

## Install

```toml
[dependencies]
vld = "0.5"
vld-tower = "0.5"
```

## Example

```rust
use axum::routing::post;
use axum::{Extension, Router};
use vld_tower::{ValidateJsonLayer, Validated};

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CreateUser {
        pub name: String  => vld::string().min(2),
        pub email: String => vld::string().email(),
    }
}

// Non-JSON requests pass the layer unvalidated, hence the Option.
async fn create(user: Option<Extension<Validated<CreateUser>>>) -> String {
    match user {
        Some(Extension(Validated(u))) => format!("created {}", u.name),
        None => "expected JSON".into(),
    }
}

let app: Router = Router::new()
    .route("/users", post(create))
    .layer(ValidateJsonLayer::<CreateUser>::new());
```

Invalid or malformed JSON is a 422 JSON response, a body over the limit (2 MiB, or `ValidateJsonLayer::with_limit`) a 413. Outside Axum, read the value with `vld_tower::validated::<T>(&req)`, which returns `Option<T>`.

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/web/tower>
- API: <https://docs.rs/vld-tower>

MSRV: Rust 1.85, see [MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md).

## License

MIT
