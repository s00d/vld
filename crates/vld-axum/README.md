# vld-axum

> **Deprecated.** `vld-axum` is not developed any more. Since vld 0.8 it is part of
> [`vld-web`](https://crates.io/crates/vld-web): `vld_web::axum`.
>
> ```toml
> vld-web = { version = "0.8", features = ["axum"] }
> ```
>
> Migration guide: <https://s00d.github.io/vld/guides/migration#from-07-to-08>. This version (0.7.1) is 0.7.0 unchanged, with this notice.

[![crates.io](https://img.shields.io/crates/v/vld-axum)](https://crates.io/crates/vld-axum)
[![docs.rs](https://img.shields.io/docsrs/vld-axum)](https://docs.rs/vld-axum)

[Axum](https://github.com/tokio-rs/axum) 0.8 extractors that validate JSON bodies, query strings, path parameters, forms, headers and cookies with [vld](https://crates.io/crates/vld) schemas.

## Install

```toml
[dependencies]
vld = "0.5"
vld-axum = "0.5"
axum = "0.8"
```

## Example

```rust
use axum::{routing::post, Router};
use vld_axum::VldJson;

vld::schema! {
    #[derive(Debug)]
    pub struct CreateUser {
        pub name: String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}

async fn create_user(VldJson(user): VldJson<CreateUser>) -> String {
    format!("created {}", user.name)
}

let app: Router = Router::new().route("/users", post(create_user));
```

Invalid input is answered with **422** and `{"error": "Validation failed", "issues": [{"path": ".name", "message": "...", "code": "too_small"}]}`; a wrong `Content-Type` is **415**, a body over axum's `DefaultBodyLimit` (2 MiB) is **413**.

Extractors: `VldJson`, `VldQuery`, `VldPath`, `VldForm`, `VldHeaders`, `VldCookie`.

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/web/axum>
- Shared adapter rules: <https://s00d.github.io/vld/integrations/web/overview>
- API: <https://docs.rs/vld-axum>

MSRV: 1.85 ([MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md)).

## License

MIT
