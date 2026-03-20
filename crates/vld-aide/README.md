[![Crates.io](https://img.shields.io/crates/v/vld-aide?style=for-the-badge)](https://crates.io/crates/vld-aide)
[![docs.rs](https://img.shields.io/docsrs/vld-aide?style=for-the-badge)](https://docs.rs/vld-aide)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=for-the-badge)](https://github.com/s00d/vld)
[![GitHub issues](https://img.shields.io/badge/github-issues-orange?style=for-the-badge)](https://github.com/s00d/vld/issues)
[![GitHub stars](https://img.shields.io/badge/github-stars-yellow?style=for-the-badge)](https://github.com/s00d/vld/stargazers)

# vld-aide

[aide](https://docs.rs/aide) / [schemars](https://docs.rs/schemars) integration for the [vld](https://crates.io/crates/vld) validation library.

## Overview

Bridge between `vld` validation schemas and `aide` OpenAPI documentation generator.
Define your validation rules once in `vld` and get `schemars::JsonSchema` compatibility
for free — no need to duplicate with `#[derive(JsonSchema)]`.

`aide` uses `schemars` for JSON Schema generation. This crate converts `vld`'s JSON Schema
output to `schemars::Schema`, making your validated types usable with `aide::axum::Json<T>`,
`aide::axum::Query<T>`, and other aide extractors.

## Installation

```toml
[dependencies]
vld = { version = "0.3", features = ["openapi"] }
vld-aide = "0.3"
aide = { version = "0.15", features = ["axum"] }
```

## Quick Start

### `impl_json_schema!` macro

```rust
use vld::prelude::*;
use vld_aide::impl_json_schema;

vld::schema! {
    #[derive(Debug)]
    pub struct CreateUser {
        pub name: String => vld::string().min(2).max(100),
        pub email: String => vld::string().email(),
        pub age: i64 => vld::number().int().min(13).max(150),
    }
}

impl_json_schema!(CreateUser);

// Now `CreateUser` implements `schemars::JsonSchema`
// and works with aide for OpenAPI doc generation.
```

### Custom schema name

```rust
impl_json_schema!(CreateUser, "CreateUserRequest");
```

### With `#[derive(Validate)]`

```rust
use vld::Validate;
use vld_aide::impl_json_schema;

#[derive(Debug, serde::Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
struct UpdateUser {
    #[vld(vld::string().min(1).max(255))]
    first_name: String,
    #[vld(vld::string().email())]
    email_address: String,
    #[vld(vld::boolean())]
    is_active: bool,
}

impl_json_schema!(UpdateUser);
// Schema properties will use camelCase: firstName, emailAddress, isActive
```

## Usage with aide + axum

```rust
use aide::axum::{ApiRouter, routing::post_with};
use aide::axum::Json;
use aide::transform::TransformOperation;

async fn create_user(Json(body): Json<CreateUser>) -> Json<serde_json::Value> {
    Json(serde_json::json!({"id": 1, "name": body.name}))
}

fn create_user_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create a new user")
      .description("Validates name, email, and age constraints.")
}

let app = ApiRouter::new()
    .api_route("/users", post_with(create_user, create_user_docs));
```

## Direct conversion

Convert any `vld` JSON schema value to `schemars::Schema`:

```rust
use vld_aide::vld_to_schemars;

let vld_schema = serde_json::json!({
    "type": "object",
    "required": ["name"],
    "properties": {
        "name": { "type": "string", "minLength": 1 }
    }
});

let schemars_schema = vld_to_schemars(&vld_schema);
```

## API Reference

| Item | Description |
|---|---|
| `impl_json_schema!(Type)` | Implement `schemars::JsonSchema` for a vld type |
| `impl_json_schema!(Type, "Name")` | Same, with custom schema name |
| `vld_to_schemars(&Value)` | Convert `serde_json::Value` to `schemars::Schema` |

## Comparison with vld-utoipa

| | `vld-utoipa` | `vld-aide` |
|---|---|---|
| Target library | [utoipa](https://docs.rs/utoipa) | [aide](https://docs.rs/aide) / [schemars](https://docs.rs/schemars) |
| Schema trait | `utoipa::ToSchema` | `schemars::JsonSchema` |
| Macro | `impl_to_schema!` | `impl_json_schema!` |
| OpenAPI version | 3.0 / 3.1 | 3.1 |

## Running the example

```sh
cargo run -p vld-aide --example aide_basic
```

## License

MIT
