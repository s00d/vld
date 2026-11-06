# vld-schemars

[![crates.io](https://img.shields.io/crates/v/vld-schemars.svg)](https://crates.io/crates/vld-schemars)
[![docs.rs](https://img.shields.io/docsrs/vld-schemars)](https://docs.rs/vld-schemars)

Two-way bridge between [vld](https://crates.io/crates/vld) and [schemars](https://crates.io/crates/schemars): vld types as `schemars::JsonSchema`, and schemars types validated through vld.

## Install

```toml
[dependencies]
vld = { version = "0.5", features = ["openapi"] }
vld-schemars = "0.5"
```

## Features

| Feature | Default | Description |
|---|---|---|
| `schemars-0` | yes | schemars `>=0.9, <1` |
| `schemars-1` | no | schemars 1.x (`default-features = false, features = ["schemars-1"]`) |

Enable exactly one. The selected crate is re-exported as `vld_schemars::schemars`.

## Example

```rust
use vld_schemars::prelude::*;
use vld_schemars::schemars;

// vld -> schemars
vld::schema! {
    #[derive(Debug)]
    pub struct User {
        pub name: String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}
impl_json_schema!(User);

// schemars -> vld
#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct Product { name: String, price: f64 }
impl_vld_parse!(Product);

let p = Product::vld_parse(&serde_json::json!({"name": "Widget", "price": 9.99})).unwrap();
```

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/openapi/schemars>
- API: <https://docs.rs/vld-schemars>

MSRV: 1.85, see [MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md).

## License

MIT
