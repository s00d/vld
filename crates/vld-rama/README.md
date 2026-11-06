# vld-rama

[![crates.io](https://img.shields.io/crates/v/vld-rama.svg)](https://crates.io/crates/vld-rama)
[![docs.rs](https://img.shields.io/docsrs/vld-rama)](https://docs.rs/vld-rama)

[Rama](https://ramaproxy.org/) extractors (`VldJson`, `VldQuery`, `VldPath`, `VldForm`, `VldHeaders`, `VldCookie`) and a `ValidateJsonLayer` for the [vld](https://crates.io/crates/vld) validation library.

## Install

```toml
[dependencies]
vld = "0.5"
vld-rama = "0.5"
rama = { version = "0.4", features = ["http"] }
```

## Example

```rust
use rama::http::service::web::WebService;
use vld_rama::VldJson;

vld::schema! {
    #[derive(Debug)]
    pub struct CreateUser {
        pub name: String => vld::string().min(2),
        pub email: String => vld::string().email(),
    }
}

async fn create(VldJson(user): VldJson<CreateUser>) -> String {
    format!("created {}", user.name)
}

let svc = WebService::default().with_post("/users", create);
```

Invalid input is a 422 JSON response, a body over 2 MiB a 413, a missing JSON `Content-Type` a 415.

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/web/rama>
- API: <https://docs.rs/vld-rama>

MSRV: Rust 1.96 (Rama's own), see [MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md).

## License

MIT
