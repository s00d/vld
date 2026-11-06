# vld-fake

[![crates.io](https://img.shields.io/crates/v/vld-fake.svg)](https://crates.io/crates/vld-fake)
[![docs.rs](https://img.shields.io/docsrs/vld-fake)](https://docs.rs/vld-fake)

Generate random test data that passes [vld](https://crates.io/crates/vld) schemas (or any JSON Schema): formats, patterns, bounds, unions, tuples and `$ref`s, with optional seeds.

## Install

```toml
[dev-dependencies]
vld = { version = "0.5", features = ["openapi"] }
vld-fake = "0.5"
```

## Example

```rust
use vld::prelude::*;
use vld_fake::FakeData;

vld::schema! {
    #[derive(Debug)]
    pub struct User {
        pub name:  String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
        pub age:   i64    => vld::number().int().min(18).max(99),
    }
}
vld_fake::impl_fake!(User);

let user = User::fake();             // validated instance
let users = User::fake_many(5);
let same = User::fake_seeded(42);    // reproducible

let raw = vld_fake::fake_value(&User::json_schema()); // serde_json::Value
```

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/codegen/fake>
- API: <https://docs.rs/vld-fake>

MSRV: 1.85, see [MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md).

## License

MIT
