[![Crates.io](https://img.shields.io/crates/v/vld-surrealdb?style=for-the-badge)](https://crates.io/crates/vld-surrealdb)
[![docs.rs](https://img.shields.io/docsrs/vld-surrealdb?style=for-the-badge)](https://docs.rs/vld-surrealdb)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=for-the-badge)](https://github.com/s00d/vld)

# vld-surrealdb

[SurrealDB](https://surrealdb.com/) integration for the [vld](https://crates.io/crates/vld) validation library.

## Overview

Validate JSON documents **before** sending to SurrealDB (`create`, `insert`, `update`) and
**after** receiving (`select`, `query`).

Zero dependency on the `surrealdb` crate — works purely through `serde`, so it's compatible
with any SurrealDB SDK version (2.x, 3.x, etc.). SurrealDB is a JSON-native document database,
making `vld` validation a perfect fit.

## Features

- **`validate_content`** — validate a struct before `db.create()` / `db.insert()` / `db.update()`
- **`validate_json`** — validate raw `serde_json::Value` from SurrealQL query results
- **`validate_record` / `validate_records`** — validate data after `db.select()`
- **`validate_value`** — validate individual field values against a schema
- **`validate_fields!`** — inline macro for multi-field validation
- **`Validated<S, T>`** — wrapper that proves the inner value has been validated
- **`VldText<S>`, `VldInt<S>`, `VldFloat<S>`, `VldBool<S>`** — typed field wrappers with validating `Deserialize`
- **`VldSurrealResponse`** — serializable error for API responses

## Installation

```toml
[dependencies]
vld = "0.1"
vld-surrealdb = "0.1"
surrealdb = "2"   # or "3" — vld-surrealdb works with any version
```

## Quick Start

### Validate before create/insert

```rust
use vld_surrealdb::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct PersonSchema {
        pub name: String  => vld::string().min(1).max(100),
        pub email: String => vld::string().email(),
        pub age: i64      => vld::number().int().min(0).max(150),
    }
}

#[derive(serde::Serialize)]
struct Person { name: String, email: String, age: i64 }

let person = Person {
    name: "Alice".into(),
    email: "alice@example.com".into(),
    age: 30,
};

// Validate, then create
validate_content::<PersonSchema, _>(&person).unwrap();
// db.create("person").content(person).await?;
```

### Validate raw JSON from queries

```rust
let result = serde_json::json!({"name": "Bob", "email": "bob@example.com", "age": 25});
validate_json::<PersonSchema>(&result).unwrap();
```

### Validated wrapper

```rust
let v = Validated::<PersonSchema, _>::new(person).unwrap();
// v implements Serialize — pass directly to SurrealDB
// db.create("person").content(&*v).await?;
println!("name={}, age={}", v.name, v.age);
```

### Typed field wrappers in documents

```rust
vld::schema! {
    #[derive(Debug)]
    pub struct EmailField {
        pub value: String => vld::string().email(),
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct UserDoc {
    name: String,
    email: VldText<EmailField>,  // validates on deserialize from SurrealDB
}
```

### Inline field validation with macro

```rust
use vld_surrealdb::validate_fields;

let name = "Alice";
let age = 30i64;

validate_fields! {
    name => vld::string().min(1).max(100),
    age => vld::number().int().min(0).max(150),
}?;
// All fields valid — proceed with DB operation
```

### Error response for APIs

```rust
match validate_content::<PersonSchema, _>(&bad_data) {
    Ok(()) => { /* proceed */ }
    Err(e) => {
        let response = VldSurrealResponse::from_error(&e);
        let json = response.to_json();
        // {"error": "Validation failed", "fields": [{"field": "email", "message": "..."}]}
    }
}
```

## API Reference

| Function | Description |
|---|---|
| `validate_content::<S, T>(&value)` | Validate struct before create/insert/update |
| `validate_json::<S>(&json)` | Validate raw JSON value |
| `validate_record::<S, T>(&value)` | Validate struct after select |
| `validate_records::<S, T>(&[values])` | Validate batch, returns `(index, error)` on failure |
| `validate_value(&schema, &json)` | Validate single field value |
| `validate_fields!{ field => schema, ... }` | Inline multi-field validation |
| `Validated::<S, T>::new(value)` | Wrap with validation proof, implements `Serialize` |
| `VldText::<S>::new(string)` | Validated text with `Serialize`/`Deserialize` |
| `VldInt::<S>::new(i64)` | Validated integer with `Serialize`/`Deserialize` |
| `VldFloat::<S>::new(f64)` | Validated float with `Serialize`/`Deserialize` |
| `VldBool::<S>::new(bool)` | Validated boolean with `Serialize`/`Deserialize` |
| `VldSurrealResponse::from_error(&e)` | Serializable error for API responses |

## Running the example

```sh
cargo run -p vld-surrealdb --example surrealdb_basic
```

## License

MIT
