# vld-aide

[![crates.io](https://img.shields.io/crates/v/vld-aide.svg)](https://crates.io/crates/vld-aide)
[![docs.rs](https://img.shields.io/docsrs/vld-aide)](https://docs.rs/vld-aide)

Use [vld](https://crates.io/crates/vld) types in [aide](https://crates.io/crates/aide) OpenAPI documents. An alias of [vld-schemars](https://crates.io/crates/vld-schemars) (`impl_json_schema!`, `vld_to_schemars`, `schemars`) with no `aide` dependency of its own: add the aide version you use.

## Install

```toml
[dependencies]
vld = { version = "0.5", features = ["openapi"] }
vld-aide = "0.5"
aide = { version = "0.15", features = ["axum"] }
```

## Features

| Feature | Default | Description |
|---|---|---|
| `schemars-0` | yes | schemars `>=0.9, <1` (aide 0.15) |
| `schemars-1` | no | schemars 1.x |

Enable exactly one, matching the schemars major of your aide version.

## Example

```rust
use vld::prelude::*;
use vld_aide::impl_json_schema;

vld::schema! {
    #[derive(Debug)]
    pub struct CreateUser {
        pub name: String => vld::string().min(2).max(100),
        pub email: String => vld::string().email(),
    }
}

impl_json_schema!(CreateUser); // CreateUser: schemars::JsonSchema
```

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/openapi/aide>
- API: <https://docs.rs/vld-aide>

MSRV: 1.85, see [MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md).

## License

MIT
