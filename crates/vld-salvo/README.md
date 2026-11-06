# vld-salvo

[![crates.io](https://img.shields.io/crates/v/vld-salvo.svg)](https://crates.io/crates/vld-salvo)
[![docs.rs](https://img.shields.io/docsrs/vld-salvo)](https://docs.rs/vld-salvo)

[Salvo](https://salvo.rs) 1.x extractors (`VldJson`, `VldQuery`, `VldForm`, `VldPath`, `VldHeaders`, `VldCookie`) for the [vld](https://crates.io/crates/vld) validation library, usable directly as `#[handler]` parameters.

## Install

```toml
[dependencies]
vld = "0.5"
vld-salvo = "0.5"
salvo = "1"
```

## Example

```rust
use salvo::prelude::*;
use vld_salvo::prelude::*;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CreateUser {
        pub name: String  => vld::string().min(2),
        pub email: String => vld::string().email(),
    }
}

#[handler]
async fn create(body: VldJson<CreateUser>, res: &mut Response) {
    res.render(Json(serde_json::json!({ "name": body.name })));
}

let router = Router::with_path("users").post(create);
```

Invalid input is a 422 JSON response, a body over the limit a 413, a missing JSON `Content-Type` a 415. The limit is Salvo's `secure_max_size`, or 2 MiB while that is Salvo's 64 KiB default.

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/web/salvo>
- API: <https://docs.rs/vld-salvo>

MSRV: Rust 1.94 (Salvo 1.x), see [MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md).

## License

MIT
