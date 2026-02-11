# vld-utoipa

Bridge between [vld](https://crates.io/crates/vld) validation library and
[utoipa](https://crates.io/crates/utoipa) OpenAPI documentation.

Define validation rules once with `vld` and automatically get `utoipa::ToSchema`
implementation — no need to duplicate schema definitions.

## Installation

```toml
[dependencies]
vld = { version = "0.1", features = ["openapi"] }
vld-utoipa = "0.1"
utoipa = "5"
```

## Quick Start

```rust
use vld::prelude::*;
use vld_utoipa::impl_to_schema;

// 1. Define validated struct as usual
vld::schema! {
    #[derive(Debug)]
    pub struct CreateUser {
        pub name: String => vld::string().min(2).max(100),
        pub email: String => vld::string().email(),
        pub age: Option<i64> => vld::number().int().gte(0).optional(),
    }
}

// 2. One line to bridge to utoipa
impl_to_schema!(CreateUser);

// Now CreateUser implements utoipa::ToSchema and can be used in
// #[utoipa::path(post, path = "/users", request_body = CreateUser)]
```

## Custom Schema Name

```rust
impl_to_schema!(CreateUser, "CreateUserRequest");
```

## Converting Arbitrary JSON Schema

```rust
use vld_utoipa::json_schema_to_schema;

let json_schema = serde_json::json!({
    "type": "object",
    "required": ["name"],
    "properties": {
        "name": { "type": "string", "minLength": 1 }
    }
});

let utoipa_schema = json_schema_to_schema(&json_schema);
```

## Supported JSON Schema Features

- Primitive types: `string`, `number`, `integer`, `boolean`, `null`
- Object with `properties` and `required`
- Array with `items`, `minItems`, `maxItems`
- `oneOf`, `allOf` composites
- `enum` values
- String: `minLength`, `maxLength`, `pattern`, `format`
- Number: `minimum`, `maximum`, `exclusiveMinimum`, `exclusiveMaximum`, `multipleOf`
- `$ref` references
- `description`, `default`, `example`, `title`

## Running the Example

```bash
cargo run -p vld-utoipa --example utoipa_basic
```

## License

MIT
