# vld-sea

[![crates.io](https://img.shields.io/crates/v/vld-sea.svg)](https://crates.io/crates/vld-sea)
[![docs.rs](https://docs.rs/vld-sea/badge.svg)](https://docs.rs/vld-sea)

[SeaORM](https://www.sea-ql.org/SeaORM/) integration for [vld](https://crates.io/crates/vld): validate `ActiveModel` values before insert / update, manually or automatically in `before_save`.

## Install

```toml
[dependencies]
vld = "0.5"
vld-sea = "0.5"
sea-orm = { version = "1", features = ["macros"] }
```

## Features

| Feature | Default | Effect |
|---|---|---|
| `sea-orm-1` | yes | SeaORM 1 |
| `sea-orm-2` | | SeaORM 2 (Rust 1.94+) |
| `with-json`, `with-chrono`, `with-time`, `with-uuid`, `with-rust_decimal`, `with-bigdecimal`, `postgres-array` | | Map these column types to JSON (also enable the SeaORM feature) |

Enable exactly one of `sea-orm-1` / `sea-orm-2`. Column types without their `with-*` feature are validated as `null`.

## Example

```rust
use vld_sea::prelude::*;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct UserInput {
        pub name: String  => vld::string().min(1).max(100),
        pub email: String => vld::string().email(),
    }
}

// Manually:
vld_sea::validate_active::<UserInput, _>(&active_model)?;

// Or on every insert / update, in the entity module:
vld_sea::impl_vld_before_save!(ActiveModel, super::UserInput);
```

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/database/sea-orm>
- API: <https://docs.rs/vld-sea>

MSRV: 1.85 (`sea-orm-2`: 1.94), see [MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md).

## License

MIT
