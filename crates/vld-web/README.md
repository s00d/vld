[![Crates.io](https://img.shields.io/crates/v/vld-web?style=for-the-badge)](https://crates.io/crates/vld-web)
[![Downloads](https://img.shields.io/crates/d/vld-web?style=for-the-badge)](https://crates.io/crates/vld-web)
[![docs.rs](https://img.shields.io/badge/docs-docs.rs-blue?style=for-the-badge)](https://docs.rs/vld-web)
[![Guide](https://img.shields.io/badge/guide-s00d.github.io-blue?style=for-the-badge)](https://s00d.github.io/vld/integrations/web/overview)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Donate](https://img.shields.io/badge/Donate-Donationalerts-ff4081?style=for-the-badge)](https://www.donationalerts.com/r/s00d88)

# vld-web

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
| `axum` | `axum` | axum 0.8 (also loco-rs controllers: [guide](https://s00d.github.io/vld/integrations/web/loco)) |
| `actix` | `actix` | actix-web 4 |
| `ntex3` / `ntex4` | `ntex-3` / `ntex-4` (+ `ntex-compress`) | ntex 3 / 4 |
| `poem` | `poem` | poem 3 |
| `rocket` | `rocket` | Rocket 0.5 |
| `salvo` | `salvo` | salvo 1 |
| `warp` | `warp` | warp 0.3 – 0.4 |
| `rama` | `rama` | rama 0.4 |
| `tower` | `tower` | any Tower service |
| `tonic` | `tonic` | tonic 0.12 – 0.14 |
| `async_graphql` | `async-graphql` | async-graphql 7 |
| `tungstenite` | `tungstenite` | tungstenite 0.26 – 0.30 WebSocket messages (tokio-/async-tungstenite) |
| `axum::ws` | `axum-ws` | axum WebSocket messages |
| `lambda` | `lambda` | AWS Lambda (`lambda_http` 1, raw `lambda_runtime` events) |
| `rmcp` | `rmcp` | MCP tools (rmcp 3): vld-validated arguments, `isError` results with model feedback |
| `rig` | `rig` | rig 0.47 tools: vld-validated arguments, schema per LLM provider |
| `common` | — | framework-agnostic helpers for custom adapters |

No feature is on by default; any set can be enabled together.

- Guide: <https://s00d.github.io/vld/integrations/web/overview>
- API: <https://docs.rs/vld-web>

## Support

If vld helps you, you can support it via [DonationAlerts](https://www.donationalerts.com/r/s00d88).
Bugs and ideas: [issues](https://github.com/s00d/vld/issues).

## License

MIT
