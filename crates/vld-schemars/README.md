[![Crates.io](https://img.shields.io/crates/v/vld-schemars?style=for-the-badge)](https://crates.io/crates/vld-schemars)
[![docs.rs](https://img.shields.io/docsrs/vld-schemars?style=for-the-badge)](https://docs.rs/vld-schemars)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)

# vld-schemars

Bidirectional bridge between [vld](https://crates.io/crates/vld) and [schemars](https://crates.io/crates/schemars).

## Overview

Many Rust libraries already use `schemars` for JSON Schema generation — aide, paperclip, okapi,
dropshot, etc. This crate lets you **share** schema definitions between `vld` and the broader
`schemars` ecosystem in **both** directions:

| Direction | Function | Description |
|---|---|---|
| **vld → schemars** | `vld_to_schemars()` | Convert vld JSON Schema to `schemars::Schema` |
| **vld → schemars** | `impl_json_schema!()` | Implement `schemars::JsonSchema` for vld types |
| **schemars → vld** | `schemars_to_json()` | Convert `schemars::Schema` to `serde_json::Value` |
| **schemars → vld** | `generate_from_schemars::<T>()` | Get JSON Schema value from any `schemars::JsonSchema` type |

Plus introspection, comparison, and schema merge utilities.

### Difference from vld-aide

`vld-aide` is specifically for the [aide](https://docs.rs/aide) OpenAPI framework.
`vld-schemars` is a **general-purpose** bridge usable with any library in the schemars ecosystem.

## Installation

```toml
[dependencies]
vld = { version = "0.1", features = ["openapi"] }
vld-schemars = "0.1"
```

## Quick Start

### vld → schemars (implement `JsonSchema` for vld types)

```rust
use vld::prelude::*;
use vld_schemars::impl_json_schema;

vld::schema! {
    #[derive(Debug)]
    pub struct User {
        pub name: String  => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}

// One line — User now works with any schemars-based library
impl_json_schema!(User);

// Custom schema name
impl_json_schema!(User, "CreateUserRequest");
```

### vld → schemars (convert JSON value)

```rust
use vld::json_schema::JsonSchema;

let vld_json = vld::string().email().json_schema();
let schemars_schema = vld_schemars::vld_to_schemars(&vld_json);
assert_eq!(schemars_schema.get("type").unwrap(), "string");
```

### schemars → vld (extract JSON from schemars)

```rust
let schemars_schema = vld_schemars::generate_schemars::<String>();
let json_value = vld_schemars::schemars_to_json(&schemars_schema);
// Now you have a serde_json::Value JSON Schema
```

### schemars → vld (generate from any JsonSchema type)

```rust
let schema = vld_schemars::generate_from_schemars::<Vec<String>>();
// Returns serde_json::Value with the full JSON Schema
```

## Introspection

```rust
use vld::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct UserSchema {
        pub name: String  => vld::string().min(1),
        pub age: i64      => vld::number().int().min(0),
    }
}

let schema = UserSchema::json_schema();

// List all properties
for prop in vld_schemars::list_properties(&schema) {
    println!("{}: type={:?}, required={}",
        prop.name, prop.schema_type, prop.required);
}

// Check specific fields
assert!(vld_schemars::is_required(&schema, "name"));
assert_eq!(vld_schemars::schema_type(&schema), Some("object".into()));

let name = vld_schemars::get_property(&schema, "name").unwrap();
assert_eq!(name["minLength"], 1);
```

## Schema Composition

### Merge (allOf)

```rust
let a = vld_schemars::vld_to_schemars(&serde_json::json!({"properties": {"x": {"type": "string"}}}));
let b = vld_schemars::vld_to_schemars(&serde_json::json!({"properties": {"y": {"type": "integer"}}}));
let merged = vld_schemars::merge_schemas(&a, &b);
// Result: {"allOf": [a, b]}
```

### Overlay constraints

```rust
let base = serde_json::json!({
    "type": "object",
    "properties": {"name": {"type": "string"}}
});
let extra = serde_json::json!({
    "properties": {"name": {"minLength": 2}},
    "required": ["name"]
});
let result = vld_schemars::overlay_constraints(&base, &extra);
// name now has minLength=2 and is required
```

## API Reference

| Function | Description |
|---|---|
| `vld_to_schemars(&Value)` | Convert JSON value to `schemars::Schema` |
| `vld_schema_to_schemars(&Value)` | Same, convenience alias |
| `schemars_to_json(&Schema)` | Convert `schemars::Schema` to `serde_json::Value` |
| `generate_from_schemars::<T>()` | Generate JSON value from `schemars::JsonSchema` type |
| `generate_schemars::<T>()` | Generate `schemars::Schema` from `schemars::JsonSchema` type |
| `impl_json_schema!(Type)` | Implement `schemars::JsonSchema` for a vld type |
| `impl_json_schema!(Type, "Name")` | Same, with custom schema name |
| `list_properties(&Value)` | Extract property info from object schema |
| `list_properties_schemars(&Schema)` | Same, for `schemars::Schema` |
| `schema_type(&Value)` | Get the "type" field |
| `is_required(&Value, &str)` | Check if field is required |
| `get_property(&Value, &str)` | Get property sub-schema |
| `schemas_equal(&Value, &Value)` | Structural equality comparison |
| `merge_schemas(&Schema, &Schema)` | Merge via allOf |
| `overlay_constraints(&Value, &Value)` | Overlay properties/required non-destructively |

## Running the example

```sh
cargo run -p vld-schemars --example schemars_basic
```

## License

MIT
