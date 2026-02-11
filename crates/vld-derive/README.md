# vld-derive

Derive macro for the [vld](https://crates.io/crates/vld) validation library.

## Overview

Provides `#[derive(Validate)]` — a procedural macro that generates `validate()` and `is_valid()` methods for your structs based on `#[vld(...)]` field attributes.

This crate is not meant to be used directly. Enable the `derive` feature on the `vld` crate instead:

```toml
[dependencies]
vld = { version = "0.1", features = ["derive"] }
```

## Quick start

```rust
use vld::prelude::*;
use vld::Validate;

#[derive(Validate)]
struct User {
    #[vld(string().min(2).max(50))]
    name: String,
    #[vld(number().int().min(0))]
    age: i64,
    #[vld(string().email())]
    email: String,
}
```

## Serde rename support

The macro automatically respects `#[serde(rename)]` and `#[serde(rename_all)]` attributes to determine the JSON field names used during validation:

```rust
use vld::prelude::*;
use vld::Validate;
use serde::Deserialize;

#[derive(Validate, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiRequest {
    #[vld(string().min(1))]
    first_name: String,
    #[vld(string().email())]
    email_address: String,
}
```

This will expect JSON keys `firstName` and `emailAddress`.

## Examples

See the [playground example](../../examples/playground/) for a complete usage demo, including `#[derive(Validate)]`:

```bash
cargo run -p playground
```

## License

MIT
