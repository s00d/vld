[![Crates.io](https://img.shields.io/crates/v/vld-derive?style=for-the-badge)](https://crates.io/crates/vld-derive)
[![docs.rs](https://img.shields.io/docsrs/vld-derive?style=for-the-badge)](https://docs.rs/vld-derive)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=for-the-badge)](https://github.com/s00d/vld)
[![GitHub issues](https://img.shields.io/badge/github-issues-orange?style=for-the-badge)](https://github.com/s00d/vld/issues)
[![GitHub stars](https://img.shields.io/badge/github-stars-yellow?style=for-the-badge)](https://github.com/s00d/vld/stargazers)

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
