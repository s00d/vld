# vld-web

[![crates.io](https://img.shields.io/crates/v/vld-web.svg)](https://crates.io/crates/vld-web)
[![docs.rs](https://img.shields.io/docsrs/vld-web)](https://docs.rs/vld-web)

Request extractors and JSON error responses that validate with [vld](https://crates.io/crates/vld) schemas, one module per web framework.

```toml
[dependencies]
vld = "0.8"
vld-web = { version = "0.8", features = ["axum"] }
```

```rust
use axum::{routing::post, Router};
use vld_web::axum::VldJson;

vld::schema! {
    #[derive(Debug)]
    pub struct CreateUser {
        pub name: String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}

async fn create(VldJson(user): VldJson<CreateUser>) -> String {
    format!("created {}", user.name) // invalid bodies never get here: 422 / 415 / 413
}

let app: Router = Router::new().route("/users", post(create));
```

| Module | Feature | Framework |
|---|---|---|
| `axum` | `axum` | axum 0.8 |
| `actix` | `actix` | actix-web 4 |
| `ntex3` / `ntex4` | `ntex-3` / `ntex-4` (+ `ntex-compress`) | ntex 3 / 4 |
| `poem` | `poem` | poem 3 |
| `rocket` | `rocket` | Rocket 0.5 |
| `salvo` | `salvo` | salvo 1 |
| `warp` | `warp` | warp 0.3 – 0.4 |
| `rama` | `rama` | rama 0.4 |
| `tower` | `tower` | any Tower service |
| `tonic` | `tonic` | tonic 0.12 – 0.14 |
| `common` | — | framework-agnostic helpers for custom adapters |

No feature is on by default; any set can be enabled together.

- Guide: <https://s00d.github.io/vld/integrations/web/overview>
- API: <https://docs.rs/vld-web>

## License

MIT
