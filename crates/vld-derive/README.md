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
vld = { version = "0.4", features = ["derive"] }
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

## OpenAPI / utoipa integration

When the `openapi` feature is enabled on `vld`, the derive macro also generates
`json_schema()` and `to_openapi_document()` methods. This makes `#[derive(Validate)]`
fully compatible with `impl_to_schema!` from `vld-utoipa`:

```toml
[dependencies]
vld = { version = "0.4", features = ["derive", "openapi"] }
vld-utoipa = "0.4"
utoipa = "5"
```

```rust
use vld::Validate;
use vld_utoipa::impl_to_schema;

#[derive(Debug, serde::Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
struct UpdateLocationRequest {
    #[vld(vld::string().min(1).max(255))]
    name: String,
    #[vld(vld::string())]
    street_address: String,
    #[vld(vld::number().int().non_negative().min(1).max(9999))]
    street_number: i64,
    #[vld(vld::boolean())]
    is_active: bool,
}

impl_to_schema!(UpdateLocationRequest);
// Now UpdateLocationRequest implements utoipa::ToSchema
// with camelCase property names in the OpenAPI spec.
```

### Query / path parameters

Add utoipa's `#[into_params(parameter_in = Query)]` (or `Path`, `Header`, `Cookie`) on the
struct, then call `impl_to_schema!` once — same as with `vld::schema!`:

```rust
use vld::Validate;
use vld_utoipa::impl_to_schema;

#[derive(Debug, serde::Deserialize, Validate)]
#[into_params(parameter_in = Query)]
struct SearchQuery {
    #[vld(vld::string().min(1).max(200))]
    q: String,
}

impl_to_schema!(SearchQuery);
```

Unlike `vld::schema!`, the derive macro **keeps** `#[into_params]` on the struct (standard
utoipa attribute). `utoipa` must be in your crate dependencies so the attribute resolves.

For `vld::schema!`, `#[into_params]` is consumed and stripped — only the OpenAPI location hint
is preserved via `OpenApiParameterIn`.

See [vld-utoipa migration guide](../vld-utoipa/README.md#migration-from-older-apis) for
deprecated macro aliases.

## Examples

See the [playground example](../../examples/playground/) for a complete usage demo, including `#[derive(Validate)]`:

```bash
cargo run -p playground
```

## License

MIT
