# vld-rama

> **Deprecated.** `vld-rama` is not developed any more. Since vld 0.8 it is part of
> [`vld-web`](https://crates.io/crates/vld-web): `vld_web::rama`.
>
> ```toml
> vld-web = { version = "0.8", features = ["rama"] }
> ```
>
> Migration guide: <https://s00d.github.io/vld/guides/migration#from-07-to-08>. This version (0.7.1) is 0.7.0 unchanged, with this notice.

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
