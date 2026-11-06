# vld-ntex

[![crates.io](https://img.shields.io/crates/v/vld-ntex)](https://crates.io/crates/vld-ntex)
[![docs.rs](https://img.shields.io/docsrs/vld-ntex)](https://docs.rs/vld-ntex)

[ntex](https://ntex.rs/) 3 / 4 extractors that validate JSON bodies, query strings, path parameters, forms, headers and cookies with [vld](https://crates.io/crates/vld) schemas.

## Install

```toml
[dependencies]
vld = "0.5"
vld-ntex = "0.5"                                   # ntex 4
ntex = { version = "4", features = ["tokio"] }

# ntex 3:
# vld-ntex = { version = "0.5", default-features = false, features = ["ntex-3"] }
```

## Features

| Feature | Default | Description |
|---|---|---|
| `ntex-4` | yes | ntex 4 |
| `ntex-3` | no | ntex 3 |
| `compress` | no | Decode `gzip` / `deflate` / `br` request bodies |

Enable exactly one of `ntex-3` / `ntex-4`. The selected crate is re-exported as `vld_ntex::ntex`.

## Example

```rust
use ntex::web::{self, App, HttpResponse, HttpServer};
use vld_ntex::VldJson;

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

#[ntex::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(async move |_| App::new().route("/users", web::post().to(create_user)))
        .bind("127.0.0.1:8080", ntex::SharedCfg::default())? // ntex 4
        .run()
        .await
}
```

Invalid input is answered with **422** and `{"error": "Validation failed", "issues": [{"path": ".name", "message": "...", "code": "too_small"}]}`; a wrong `Content-Type` is **415**, a body over the limit (2 MiB, change it with the `VldBodyLimit` app state) is **413**.

Extractors: `VldJson`, `VldQuery`, `VldPath`, `VldForm`, `VldHeaders`, `VldCookie`.

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/web/ntex>
- Shared adapter rules: <https://s00d.github.io/vld/integrations/web/overview>
- API: <https://docs.rs/vld-ntex>

MSRV: 1.97 with `ntex-4` (default); `ntex-3` builds on 1.88 with `--ignore-rust-version` ([MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md)).

## License

MIT
