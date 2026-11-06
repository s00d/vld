# vld-actix

[![crates.io](https://img.shields.io/crates/v/vld-actix)](https://crates.io/crates/vld-actix)
[![docs.rs](https://img.shields.io/docsrs/vld-actix)](https://docs.rs/vld-actix)

[Actix-web](https://actix.rs/) 4 extractors that validate JSON bodies, query strings, path parameters, forms, headers and cookies with [vld](https://crates.io/crates/vld) schemas.

## Install

```toml
[dependencies]
vld = "0.5"
vld-actix = "0.5"
actix-web = "4"
```

## Example

```rust
use actix_web::{web, App, HttpResponse};
use vld_actix::{VldBodyLimit, VldJson};

vld::schema! {
    #[derive(Debug)]
    pub struct CreateUser {
        pub name: String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}

async fn create_user(user: VldJson<CreateUser>) -> HttpResponse {
    HttpResponse::Created().body(format!("created {}", user.name))
}

let app = App::new()
    .app_data(VldBodyLimit(8 * 1024 * 1024)) // optional, default 2 MiB
    .route("/users", web::post().to(create_user));
```

Invalid input is answered with **422** and `{"error": "Validation failed", "issues": [{"path": ".name", "message": "...", "code": "too_small"}]}`; a wrong `Content-Type` is **415**, a body over the limit is **413**. Actix's `JsonConfig` / `PayloadConfig` do not apply — use `VldBodyLimit`.

Extractors: `VldJson`, `VldQuery`, `VldPath`, `VldForm`, `VldHeaders`, `VldCookie`.

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/web/actix>
- Shared adapter rules: <https://s00d.github.io/vld/integrations/web/overview>
- API: <https://docs.rs/vld-actix>

MSRV: 1.85 ([MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md)).

## License

MIT
