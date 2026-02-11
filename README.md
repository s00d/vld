# vld

**Type-safe runtime validation for Rust, inspired by [Zod](https://zod.dev/).**

`vld` combines schema definition with type-safe parsing. Define your validation
rules once and get both runtime checks and strongly-typed Rust structs.

[![Crates.io](https://img.shields.io/crates/v/vld.svg)](https://crates.io/crates/vld)
[![Docs.rs](https://docs.rs/vld/badge.svg)](https://docs.rs/vld)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

---

## Features

- **Zero-cost schema definitions** — the `schema!` macro generates plain Rust structs with
  built-in `parse()` methods. Or use `schema_validated!` to get lenient parsing too.
- **Error accumulation** — all validation errors are collected, not just the first one.
- **Rich primitives** — string, number, integer, boolean, literal, enum, any, custom.
- **String formats** — email, URL, UUID, IPv4, IPv6, Base64, ISO date/time/datetime, hostname, CUID2, ULID, Nano ID, emoji.
  All validated without regex by default. Every check has a `_msg` variant for custom messages.
- **Composable** — optional, nullable, nullish, default, catch, refine, super_refine, transform, pipe, preprocess, describe, `.or()`, `.and()`.
- **Collections** — arrays, tuples (up to 6), records, Map (`HashMap`), Set (`HashSet`).
- **Unions** — `union(a, b)`, `union3(a, b, c)`, `.or()`, `discriminated_union("field")`, `intersection(a, b)`, `.and()`.
- **Recursive schemas** — `lazy()` for self-referencing data structures (trees, graphs).
- **Dynamic objects** — `strict()`, `strip()`, `passthrough()`, `pick()`, `omit()`, `extend()`, `merge()`, `partial()`, `required()`, `catchall()`, `keyof()`.
- **Custom schemas** — `vld::custom(|v| ...)` for arbitrary validation logic.
- **Multiple input sources** — parse from `&str`, `String`, `&[u8]`, `Path`, `PathBuf`, or `serde_json::Value`.
- **Validate existing values** — `.validate(&value)` and `.is_valid(&value)` work with any `Serialize` type. `schema!` structs get `Struct::validate(&instance)`.
- **Lenient parsing** — `parse_lenient()` returns `ParseResult<T>` with the struct, per-field diagnostics, and `.save_to_file()`.
- **Error formatting** — `prettify_error`, `flatten_error`, `treeify_error` utilities.
- **Custom error messages** — `_msg` variants, `type_error()`, and `with_messages()` for per-check and bulk message overrides, including translations.
- **JSON Schema / OpenAPI** — `JsonSchema` trait on all schema types; `json_schema()` and `to_openapi_document()` on `schema!` structs; `field_schema()` for rich object property schemas; `to_openapi_document_multi()` helper.
- **Derive macro** — `#[derive(Validate)]` with `#[vld(...)]` attributes (optional `derive` feature).
- **Benchmarks** — criterion-based benchmarks included.
- **CI** — GitHub Actions workflow for testing, clippy, and formatting.
- **Minimal dependencies** — only `serde` and `serde_json`. Regex and derive macro are optional features.

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
vld = "0.1"
```

### Optional Features

All features are **disabled** by default:

| Feature       | Description                                                                                                                                                         |
|---------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `serialize`   | Adds `#[derive(Serialize)]` on error/result types, enables `VldSchema::validate()`/`is_valid()`, `ParseResult::save_to_file()`/`to_json_string()`/`to_json_value()` |
| `deserialize` | Adds `#[derive(Deserialize)]` on error/result types                                                                                                                 |
| `openapi`     | Enables `JsonSchema` trait, `to_json_schema()`, `json_schema()`, `to_openapi_document()`, `field_schema()`                                                          |
| `diff`        | Schema diffing — compare two JSON Schemas to detect breaking vs non-breaking changes                                                                                |
| `regex`       | Custom regex patterns via `.regex()` (uses `regex-lite`)                                                                                                            |
| `derive`      | `#[derive(Validate)]` procedural macro                                                                                                                              |
| `chrono`      | `ZDate` / `ZDateTime` types with `chrono` parsing                                                                                                                   |

Enable features as needed:

```toml
[dependencies]
vld = { version = "0.1", features = ["serialize", "openapi"] }
```

### Basic Usage

```rust
use vld::prelude::*;

// Define a validated struct
vld::schema! {
    #[derive(Debug)]
    pub struct User {
        pub name: String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
        pub age: Option<i64> => vld::number().int().gte(18).optional(),
    }
}

// Parse from a JSON string
let user = User::parse(r#"{"name": "Alex", "email": "alex@example.com"}"#).unwrap();
assert_eq!(user.name, "Alex");
assert_eq!(user.age, None);

// Errors are accumulated
let err = User::parse(r#"{"name": "A", "email": "bad"}"#).unwrap_err();
assert!(err.issues.len() >= 2);
```

### Nested Structs

```rust
use vld::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct Address {
        pub city: String => vld::string().min(1),
        pub zip: String  => vld::string().len(6),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct User {
        pub name: String       => vld::string().min(2),
        pub address: Address   => vld::nested(Address::parse_value),
    }
}

let user = User::parse(r#"{
    "name": "Alex",
    "address": {"city": "New York", "zip": "100001"}
}"#).unwrap();
```

## Primitives

### String

```rust
vld::string()
    .min(3)                 // minimum length
    .max(100)               // maximum length
    .len(10)                // exact length
    .email()                // email format
    .url()                  // URL format (http/https)
    .uuid()                 // UUID format
    .ipv4()                 // IPv4 address
    .ipv6()                 // IPv6 address
    .base64()               // Base64 string
    .iso_date()             // ISO 8601 date (YYYY-MM-DD)
    .iso_time()             // ISO 8601 time (HH:MM:SS)
    .iso_datetime()         // ISO 8601 datetime
    .hostname()             // valid hostname
    .cuid2()                // CUID2 format
    .ulid()                 // ULID format (26 chars, Crockford Base32)
    .nanoid()               // Nano ID format (alphanumeric + _-)
    .emoji()                // must contain emoji
    .starts_with("prefix")  // must start with
    .ends_with("suffix")    // must end with
    .contains("sub")        // must contain
    .non_empty()            // must not be empty
    .trim()                 // trim whitespace before validation
    .to_lowercase()         // convert to lowercase
    .to_uppercase()         // convert to uppercase
    .coerce()               // coerce numbers/booleans to string
```

### Number

```rust
vld::number()
    .min(0.0)       // minimum (inclusive)
    .max(100.0)     // maximum (inclusive)
    .gt(0.0)        // greater than (exclusive)
    .lt(100.0)      // less than (exclusive)
    .positive()     // > 0
    .negative()     // < 0
    .non_negative() // >= 0
    .finite()       // not NaN or infinity
    .multiple_of(5.0)
    .safe()         // JS safe integer range (-(2^53-1) to 2^53-1)
    .int()          // switch to integer mode (i64)
    .coerce()       // coerce strings/booleans to number
```

### Integer

```rust
vld::number().int()
    .min(0)
    .max(100)
    .gte(18)
    .positive()
```

### Boolean

```rust
vld::boolean()
    .coerce()  // "true"/"false"/"1"/"0" -> bool
```

### Literal

```rust
vld::literal("admin")   // exact string match
vld::literal(42i64)      // exact integer match
vld::literal(true)       // exact boolean match
```

### Enum

```rust
vld::enumeration(&["admin", "user", "moderator"])
```

### Any

```rust
vld::any()  // accepts any JSON value
```

## Modifiers

```rust
// Optional: null/missing -> None
vld::string().optional()

// Nullable: null -> None
vld::string().nullable()

// Nullish: both optional + nullable
vld::string().nullish()

// Default: null/missing -> default value
vld::string().with_default("fallback".to_string())

// Catch: ANY error -> fallback value
vld::string().min(3).catch("default".to_string())
```

## Collections

### Array

```rust
vld::array(vld::string().non_empty())
    .min_len(1)
    .max_len(10)
    .len(5)       // exact length
    .non_empty()  // alias for min_len(1)
```

### Tuple

```rust
// Tuples of 1-6 elements
let schema = (vld::string(), vld::number().int(), vld::boolean());
let (s, n, b) = schema.parse(r#"["hello", 42, true]"#).unwrap();
```

### Record

```rust
vld::record(vld::number().int().positive())
    .min_keys(1)
    .max_keys(10)
```

### Map

```rust
// Input: [["a", 1], ["b", 2]] -> HashMap
vld::map(vld::string(), vld::number().int())
```

### Set

```rust
// Input: ["a", "b", "a"] -> HashSet {"a", "b"}
vld::set(vld::string().min(1))
    .min_size(1)
    .max_size(10)
```

## Combinators

### Union

```rust
// Union of 2 types
let schema = vld::union(vld::string(), vld::number().int());
// Returns Either<String, i64>

// Union of 3 types
let schema = vld::union3(vld::string(), vld::number(), vld::boolean());
// Returns Either3<String, f64, bool>
```

### `union!` macro

For convenience, use the `union!` macro to combine 2–6 schemas without
nesting calls manually. The macro dispatches to `union()` / `union3()` or
nests them automatically for higher arities:

```rust
use vld::prelude::*;

// 2 schemas — same as vld::union(a, b)
let s2 = vld::union!(vld::string(), vld::number());

// 3 schemas — same as vld::union3(a, b, c)
let s3 = vld::union!(vld::string(), vld::number(), vld::boolean());

// 4 schemas — nested automatically
let s4 = vld::union!(
    vld::string(),
    vld::number(),
    vld::boolean(),
    vld::number().int(),
);

// 5 and 6 schemas work the same way
let s5 = vld::union!(
    vld::string(),
    vld::number(),
    vld::boolean(),
    vld::number().int(),
    vld::literal("hello"),
);
```

You can also use the method chaining equivalent `.or()` for two schemas:

```rust
let schema = vld::string().or(vld::number().int());
```

### Discriminated Union

```rust
// Efficient union by discriminator field
let schema = vld::discriminated_union("type")
    .variant_str("dog", vld::object()
        .field("type", vld::literal("dog"))
        .field("bark", vld::boolean()))
    .variant_str("cat", vld::object()
        .field("type", vld::literal("cat"))
        .field("lives", vld::number().int()));
```

### Intersection

```rust
// Input must satisfy both schemas
let schema = vld::intersection(
    vld::string().min(3),
    vld::string().email(),
);
```

### Refine

```rust
vld::number().int().refine(|n| n % 2 == 0, "Must be even")
```

### Super Refine

```rust
// Produce multiple errors in one check
vld::string().super_refine(|s, errors| {
    if s.len() < 3 {
        errors.push(IssueCode::Custom { code: "short".into() }, "Too short");
    }
    if !s.contains('@') {
        errors.push(IssueCode::Custom { code: "no_at".into() }, "Missing @");
    }
})
```

### Transform

```rust
vld::string().transform(|s| s.len())  // String -> usize
```

### Pipe

```rust
// Chain schemas: output of first -> input of second
vld::string()
    .transform(|s| s.len())
    .pipe(vld::number().min(3.0))
```

### Preprocess

```rust
vld::preprocess(
    |v| match v.as_str() {
        Some(s) => serde_json::json!(s.trim()),
        None => v.clone(),
    },
    vld::string().min(1),
)
```

### Lazy (Recursive)

```rust
// Self-referencing schemas for trees, graphs, etc.
fn tree() -> vld::object::ZObject {
    vld::object()
        .field("value", vld::number().int())
        .field("children", vld::array(vld::lazy(tree)))
}
```

### Describe

```rust
// Attach metadata (does not affect validation)
vld::string().min(3).describe("User's full name")
```

## Dynamic Object

For runtime-defined schemas (without compile-time type safety):

```rust
let obj = vld::object()
    .field("name", vld::string().min(1))
    .field("score", vld::number().min(0.0).max(100.0))
    .strict();    // reject unknown fields
    // .strip()   // remove unknown fields (default)
    // .passthrough()  // keep unknown fields as-is

// Object manipulation
let base = vld::object().field("a", vld::string()).field("b", vld::number());
base.pick(&["a"])          // keep only "a"
base.omit("b")             // remove "b"
base.partial()             // all fields become optional
base.required()            // all fields must not be null (opposite of partial)
base.deep_partial()        // partial (nested objects: apply separately)
base.extend(other_object)  // merge fields from another schema
base.merge(other_object)   // alias for extend
base.catchall(vld::string()) // validate unknown fields with a schema
base.keyof()               // Vec<String> of field names
```

## Per-Field Validation & Lenient Parsing

Use `schema_validated!` for zero-duplication, or `schema!` + `impl_validate_fields!` separately:

```rust
use vld::prelude::*;

// Option A: single macro (requires Serialize + Default on field types)
vld::schema_validated! {
    #[derive(Debug, serde::Serialize)]
    pub struct User {
        pub name: String     => vld::string().min(2),
        pub email: String    => vld::string().email(),
        pub age: Option<i64> => vld::number().int().gte(18).optional(),
    }
}

// Option B: separate macros (more control)
// vld::schema! { ... }
// vld::impl_validate_fields!(User { name: String => ..., });
```

### validate_fields — per-field diagnostics

```rust
let results = User::validate_fields(r#"{"name": "X", "email": "bad"}"#).unwrap();
for f in &results {
    println!("{}", f);
}
// Output:
//   ✖ name: String must be at least 2 characters (received: "X")
//   ✖ email: Invalid email address (received: "bad")
//   ✔ age: null
```

### parse_lenient — returns a `ParseResult<T>`

`parse_lenient` returns a [`ParseResult<T>`] — a wrapper around the struct and
per-field diagnostics. You can inspect it, convert to JSON, or save to a file
**whenever you want**.

```rust
let result = User::parse_lenient(r#"{"name": "X", "email": "bad"}"#).unwrap();

// Inspect
println!("valid: {}", result.is_valid());        // false
println!("errors: {}", result.error_count());     // 2
println!("value: {:?}", result.value);            // User { name: "", email: "", age: None }

// Per-field diagnostics
for f in result.fields() {
    println!("{}", f);
}

// Only errors
for f in result.error_fields() {
    println!("{}", f);
}

// Display trait prints a summary
println!("{}", result);

// Convert to JSON string
let json = result.to_json_string().unwrap();

// Save to file at any time
result.save_to_file(std::path::Path::new("output.json")).unwrap();

// Or extract the struct
let user = result.into_value();
```

**`ParseResult<T>` methods:**

| Method                | Description                                           |
|-----------------------|-------------------------------------------------------|
| `.value`              | The constructed struct (invalid fields use `Default`) |
| `.fields()`           | All per-field results (`&[FieldResult]`)              |
| `.valid_fields()`     | Only passed fields                                    |
| `.error_fields()`     | Only failed fields                                    |
| `.is_valid()`         | `true` if all fields passed                           |
| `.has_errors()`       | `true` if any field failed                            |
| `.valid_count()`      | Number of valid fields                                |
| `.error_count()`      | Number of invalid fields                              |
| `.save_to_file(path)` | Serialize to JSON file (requires `Serialize`)         |
| `.to_json_string()`   | Serialize to JSON string                              |
| `.to_json_value()`    | Serialize to `serde_json::Value`                      |
| `.into_value()`       | Consume and return the inner struct                   |
| `.into_parts()`       | Consume and return `(T, Vec<FieldResult>)`            |

## Single-Field Extraction

Parse the entire schema first, then extract individual fields from the result.
Use `parse_lenient` + `.field("name")` to inspect a specific field's validation
status — even when other fields are invalid:

```rust
use vld::prelude::*;

// Define and register per-field validation
vld::schema! {
    #[derive(Debug, serde::Serialize, Default)]
    pub struct User {
        pub name: String     => vld::string().min(2),
        pub email: String    => vld::string().email(),
        pub age: Option<i64> => vld::number().int().gte(18).optional(),
    }
}
vld::impl_validate_fields!(User {
    name  : String      => vld::string().min(2),
    email : String      => vld::string().email(),
    age   : Option<i64> => vld::number().int().gte(18).optional(),
});

// Strict parse — access fields directly
let user = User::parse(r#"{"name":"Alex","email":"a@b.com","age":30}"#).unwrap();
println!("{}", user.name);  // "Alex"

// Lenient parse — some fields may be invalid
let result = User::parse_lenient(r#"{"name":"X","email":"bad","age":25}"#).unwrap();

// The struct is always available (invalid fields use Default)
println!("{}", result.value.age.unwrap()); // 25 — valid, kept as-is

// Check a specific field
let name_field = result.field("name").unwrap();
println!("{}", name_field);       // ✖ name: String must be at least 2 characters
println!("{}", name_field.is_ok()); // false

let age_field = result.field("age").unwrap();
println!("{}", age_field);        // ✔ age: 25
println!("{}", age_field.is_ok()); // true
```

## Error Formatting

```rust
use vld::format::{prettify_error, flatten_error, treeify_error};

match User::parse(bad_input) {
    Err(e) => {
        // Human-readable with markers
        println!("{}", prettify_error(&e));
        // ✖ String must be at least 2 characters
        //   → at .name, received "A"

        // Flat map: field -> Vec<message>
        let flat = flatten_error(&e);
        for (field, msgs) in &flat.field_errors {
            println!("{}: {:?}", field, msgs);
        }

        // Tree structure mirroring the schema
        let tree = treeify_error(&e);
    }
    _ => {}
}
```

## Input Sources

Schemas accept any type implementing `VldInput`:

```rust
// JSON string
User::parse(r#"{"name": "Alex", "email": "a@b.com"}"#)?;

// serde_json::Value
let val = serde_json::json!({"name": "Alex", "email": "a@b.com"});
User::parse(&val)?;

// File path
User::parse(std::path::Path::new("data/user.json"))?;

// Byte slice
User::parse(b"{\"name\": \"Alex\", \"email\": \"a@b.com\"}" as &[u8])?;
```

## Validate Existing Rust Values

> Requires the `serialize` feature.

Instead of only parsing JSON, you can validate any existing Rust value using
`.validate()` and `.is_valid()`. The value is serialized to JSON internally,
then validated against the schema.

### On any schema

```rust
use vld::prelude::*;

// Validate a Vec
let schema = vld::array(vld::number().int().positive()).min_len(1).max_len(5);
assert!(schema.is_valid(&vec![1, 2, 3]));
assert!(schema.validate(&vec![-1, 0]).is_err());

// Validate a String
let email = vld::string().email();
assert!(email.is_valid(&"user@example.com"));
assert!(!email.is_valid(&"bad"));

// Validate a number
let age = vld::number().int().min(18).max(120);
assert!(age.is_valid(&25));
assert!(!age.is_valid(&10));

// Validate a HashMap
let schema = vld::record(vld::number().positive());
let mut map = std::collections::HashMap::new();
map.insert("score", 95.5);
assert!(schema.is_valid(&map));
```

### On `schema!` structs

Structs with `#[derive(serde::Serialize)]` get `validate()` and `is_valid()`
that check an already-constructed instance against the schema:

```rust
use vld::prelude::*;

vld::schema! {
    #[derive(Debug, serde::Serialize)]
    pub struct User {
        pub name: String => vld::string().min(2),
        pub email: String => vld::string().email(),
    }
}

// Construct a struct normally (bypassing parse)
let user = User {
    name: "A".to_string(),        // too short
    email: "bad".to_string(),     // invalid email
};

// Validate it
assert!(!User::is_valid(&user));
let err = User::validate(&user).unwrap_err();
// err contains: .name: too short, .email: invalid

// Also works with serde_json::Value or any Serialize type
let json = serde_json::json!({"name": "Bob", "email": "bob@test.com"});
assert!(User::is_valid(&json));
```

## `impl_rules!` — Attach Validation to Existing Structs

Use `impl_rules!` to add `.validate()` and `.is_valid()` to a struct you
already have. No need to redefine it — just list the field rules:

```rust
use vld::prelude::*;

// No #[derive(Serialize)] or #[derive(Debug)] required
struct Product {
    name: String,
    price: f64,
    quantity: i64,
    tags: Vec<String>,
}

vld::impl_rules!(Product {
    name     => vld::string().min(2).max(100),
    price    => vld::number().positive(),
    quantity => vld::number().int().non_negative(),
    tags     => vld::array(vld::string().min(1)).max_len(10),
});

let p = Product {
    name: "Widget".into(),
    price: 9.99,
    quantity: 5,
    tags: vec!["sale".into()],
};
assert!(p.is_valid());

let bad = Product {
    name: "X".into(),
    price: -1.0,
    quantity: -1,
    tags: vec!["".into()],
};
assert!(!bad.is_valid());
let err = bad.validate().unwrap_err();
for issue in &err.issues {
    let path: String = issue.path.iter().map(|p| p.to_string()).collect();
    println!("{}: {}", path, issue.message);
}
// .name: String must be at least 2 characters
// .price: Number must be positive
// .quantity: Number must be non-negative
// .tags[0]: String must be at least 1 characters
```

The struct itself does **not** need `Serialize` or `Debug` — each field is
serialized individually (standard types like `String`, `f64`, `Vec<T>` already
implement `Serialize`). You can use all schema features inside `impl_rules!`:
`with_messages()`, `type_error()`, `refine()`, etc.

## Chain Syntax: `.or()` / `.and()`

```rust
// Union via method chaining
let schema = vld::string().or(vld::number().int());
// Equivalent to vld::union(vld::string(), vld::number().int())

// Intersection via method chaining
let bounded = vld::string().min(3).and(vld::string().email());
// Input must satisfy both constraints
```

## Custom Schema

Create a schema from any closure:

```rust
let even = vld::custom(|v: &serde_json::Value| {
    let n = v.as_i64().ok_or("Expected integer")?;
    if n % 2 == 0 { Ok(n) } else { Err("Must be even".into()) }
});
assert_eq!(even.parse("4").unwrap(), 4);
assert!(even.parse("5").is_err());
```

## JSON Schema / OpenAPI Generation

> Requires the `openapi` feature.

Generate [JSON Schema](https://json-schema.org/) (compatible with **OpenAPI 3.1**)
from any `vld` schema via the `JsonSchema` trait:

```rust
use vld::prelude::*;  // imports JsonSchema trait

// Any individual schema
let js = vld::string().min(2).max(50).email().json_schema();
// {"type": "string", "minLength": 2, "maxLength": 50, "format": "email"}

// Collections
let js = vld::array(vld::number().int().positive()).min_len(1).json_schema();
// {"type": "array", "items": {"type": "integer", ...}, "minItems": 1}

// Modifiers (optional wraps with oneOf)
let js = vld::string().email().optional().json_schema();
// {"oneOf": [{"type": "string", "format": "email"}, {"type": "null"}]}

// Unions → oneOf, Intersections → allOf
let js = vld::union(vld::string(), vld::number()).json_schema();
// {"oneOf": [{"type": "string"}, {"type": "number"}]}
```

### Object field schemas

Use `field_schema()` (instead of `field()`) to include full JSON Schema for
each property:

```rust
let js = vld::object()
    .field_schema("email", vld::string().email().min(5))
    .field_schema("score", vld::number().min(0.0).max(100.0))
    .strict()
    .json_schema();
// {"type": "object", "properties": {"email": {...}, "score": {...}}, ...}
```

### `schema!` macro — struct-level JSON Schema

Structs defined via `schema!` automatically get `json_schema()` and
`to_openapi_document()` class methods:

```rust
use vld::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct User {
        pub name: String => vld::string().min(2).max(100),
        pub email: String => vld::string().email(),
        pub age: i64 => vld::number().int().min(0),
    }
}

// Full JSON Schema for the struct
let schema = User::json_schema();
// {
//   "type": "object",
//   "required": ["name", "email", "age"],
//   "properties": {
//     "name": {"type": "string", "minLength": 2, "maxLength": 100},
//     "email": {"type": "string", "format": "email"},
//     "age": {"type": "integer", "minimum": 0}
//   }
// }

// Wrap in a minimal OpenAPI 3.1 document
let doc = User::to_openapi_document();
// {"openapi": "3.1.0", "components": {"schemas": {"User": {...}}}, ...}
```

### Multi-schema OpenAPI document

```rust
use vld::json_schema::to_openapi_document_multi;

let doc = to_openapi_document_multi(&[
    ("User", User::json_schema()),
    ("Address", Address::json_schema()),
]);
```

### `JsonSchema` trait

The trait is implemented for all core types: `ZString`, `ZNumber`, `ZInt`,
`ZBoolean`, `ZEnum`, `ZAny`, `ZArray`, `ZRecord`, `ZSet`, `ZObject`,
`ZOptional`, `ZNullable`, `ZNullish`, `ZDefault`, `ZCatch`, `ZRefine`,
`ZTransform`, `ZDescribe`, `ZUnion2`, `ZUnion3`, `ZIntersection`,
`NestedSchema`.

## Custom Error Messages

Error messages are configured at the **schema level**, not after validation.
There are three mechanisms:

### 1. `_msg` variants — per-check custom messages

Every validation method has a `_msg` variant that accepts a custom error message:

```rust
use vld::prelude::*;

let schema = vld::string()
    .min_msg(3, "Name must be at least 3 characters")
    .max_msg(50, "Name is too long")
    .email_msg("Please enter a valid email");

let err = schema.parse(r#""ab""#).unwrap_err();
// -> "Name must be at least 3 characters"
// -> "Please enter a valid email"
```

Available on all string checks (`email_msg`, `url_msg`, `uuid_msg`, `ipv4_msg`, etc.)
and number checks are set via `with_messages` (see below).

### 2. `type_error()` — custom type mismatch message

Override the "Expected X, received Y" message when the input has the wrong JSON type:

```rust
use vld::prelude::*;

let schema = vld::string().type_error("This field requires text");
let err = schema.parse("42").unwrap_err();
assert!(err.issues[0].message.contains("This field requires text"));

let schema = vld::number().type_error("Age must be a number");
let schema = vld::number().int().int_error("Whole numbers only");
```

### 3. `with_messages()` — bulk override by check key

Override multiple messages at once using check category keys. The closure receives
the key and returns `Some(new_message)` to replace, or `None` to keep the original:

```rust
use vld::prelude::*;

let schema = vld::string().min(5).max(100).email()
    .with_messages(|key| match key {
        "too_small" => Some("Too short!".into()),
        "too_big" => Some("Too long!".into()),
        "invalid_email" => Some("Bad email!".into()),
        _ => None,
    });
```

Works on numbers too — great for translations:

```rust
use vld::prelude::*;

let schema = vld::number().min(1.0).max(100.0)
    .with_messages(|key| match key {
        "too_small" => Some("Значение должно быть не менее 1".into()),
        "too_big" => Some("Значение не должно превышать 100".into()),
        _ => None,
    });
```

For integers, the key `"not_int"` overrides the "not an integer" message:

```rust
use vld::prelude::*;

let schema = vld::number().int().min(1).max(10)
    .with_messages(|key| match key {
        "too_small" => Some("Minimum is 1".into()),
        "not_int" => Some("No decimals allowed".into()),
        _ => None,
    });
```

### 4. In objects — per-field custom messages

Combine `type_error()` and `with_messages()` on individual fields:

```rust
use vld::prelude::*;

let schema = vld::object()
    .field("name", vld::string().min(2)
        .type_error("Name must be text")
        .with_messages(|k| match k {
            "too_small" => Some("Name is too short".into()),
            _ => None,
        }))
    .field("age", vld::number().int().min(18)
        .type_error("Age must be a number")
        .with_messages(|k| match k {
            "too_small" => Some("Must be 18 or older".into()),
            _ => None,
        }));
```

### String check keys

| Key                    | Check          |
|------------------------|----------------|
| `too_small`            | `min`          |
| `too_big`              | `max`          |
| `invalid_length`       | `len`          |
| `invalid_email`        | `email`        |
| `invalid_url`          | `url`          |
| `invalid_uuid`         | `uuid`         |
| `invalid_regex`        | `regex`        |
| `invalid_starts_with`  | `starts_with`  |
| `invalid_ends_with`    | `ends_with`    |
| `invalid_contains`     | `contains`     |
| `non_empty`            | `non_empty`    |
| `invalid_ipv4`         | `ipv4`         |
| `invalid_ipv6`         | `ipv6`         |
| `invalid_base64`       | `base64`       |
| `invalid_iso_date`     | `iso_date`     |
| `invalid_iso_datetime` | `iso_datetime` |
| `invalid_iso_time`     | `iso_time`     |
| `invalid_hostname`     | `hostname`     |
| `invalid_cuid2`        | `cuid2`        |
| `invalid_ulid`         | `ulid`         |
| `invalid_nanoid`       | `nanoid`       |
| `invalid_emoji`        | `emoji`        |

### Number check keys

| Key                | Check              |
|--------------------|--------------------|
| `too_small`        | `min`, `gt`, `gte` |
| `too_big`          | `max`, `lt`, `lte` |
| `not_positive`     | `positive`         |
| `not_negative`     | `negative`         |
| `not_non_negative` | `non_negative`     |
| `not_non_positive` | `non_positive`     |
| `not_finite`       | `finite`           |
| `not_multiple_of`  | `multiple_of`      |
| `not_safe`         | `safe`             |
| `not_int`          | `int` (ZInt only)  |

## Derive Macro

Enable the `derive` feature for `#[derive(Validate)]`:

```toml
[dependencies]
vld = { version = "0.1", features = ["derive"] }
```

```rust
use vld::Validate;

#[derive(Debug, Default, serde::Serialize, Validate)]
struct User {
    #[vld(vld::string().min(2).max(50))]
    name: String,
    #[vld(vld::string().email())]
    email: String,
    #[vld(vld::number().int().gte(18).optional())]
    age: Option<i64>,
}

// Generates: parse(), parse_value(), validate_fields(), parse_lenient()
let user = User::parse(r#"{"name": "Alex", "email": "a@b.com"}"#).unwrap();
```

## Optional Regex Support

> Requires the `regex` feature.

By default, `vld` validates all string formats (email, UUID, etc.) without regex.
If you need custom regex patterns via `.regex()`, enable the `regex` feature:

```toml
[dependencies]
vld = { version = "0.1", features = ["regex"] }
```

```rust
let schema = vld::string().regex(vld::regex_lite::Regex::new(r"^\d{3}-\d{4}$").unwrap());
```

## Running the Playground

```bash
cargo run --example playground
```

## Benchmarks

```bash
cargo bench
```

## Workspace Crates

The `vld` project is organized as a Cargo workspace with several crates:

| Crate                       | Path          | Description                                                                                                                              |
|-----------------------------|---------------|------------------------------------------------------------------------------------------------------------------------------------------|
| [`vld`](.)                              | `.`                    | Core validation library — schemas, parsers, macros, error handling, i18n                                                                 |
| [`vld-derive`](crates/vld-derive/)      | `crates/vld-derive/`   | Procedural macro `#[derive(Validate)]` for automatic struct validation                                                                   |
| [`vld-axum`](crates/vld-axum/)          | `crates/vld-axum/`     | [Axum](https://github.com/tokio-rs/axum) integration — extractors `VldJson`, `VldQuery`, `VldPath`, `VldForm`, `VldHeaders`, `VldCookie` |
| [`vld-actix`](crates/vld-actix/)        | `crates/vld-actix/`    | [Actix-web](https://actix.rs/) integration — extractors `VldJson`, `VldQuery`, `VldPath`, `VldForm`, `VldHeaders`, `VldCookie`           |
| [`vld-diesel`](crates/vld-diesel/)      | `crates/vld-diesel/`   | [Diesel](https://diesel.rs/) ORM integration — `Validated<S, T>` wrapper, `VldText`, `VldInt` column types                               |
| [`vld-utoipa`](crates/vld-utoipa/)      | `crates/vld-utoipa/`   | [utoipa](https://docs.rs/utoipa) integration — auto-generate `ToSchema` from `vld` definitions                                          |
| [`vld-config`](crates/vld-config/)      | `crates/vld-config/`   | Config validation — validate TOML/YAML/JSON/ENV via [config-rs](https://docs.rs/config) or [figment](https://docs.rs/figment)            |
| [`vld-tower`](crates/vld-tower/)        | `crates/vld-tower/`    | Universal [Tower](https://docs.rs/tower) middleware — validate JSON bodies in any Tower-compatible framework                              |
| [`vld-rocket`](crates/vld-rocket/)      | `crates/vld-rocket/`   | [Rocket](https://rocket.rs/) integration — extractors `VldJson`, `VldQuery`, `VldForm` + JSON error catchers                             |
| [`vld-poem`](crates/vld-poem/)          | `crates/vld-poem/`     | [Poem](https://docs.rs/poem) integration — extractors `VldJson`, `VldQuery`, `VldForm`                                                   |
| [`vld-warp`](crates/vld-warp/)          | `crates/vld-warp/`     | [Warp](https://docs.rs/warp) integration — filters `vld_json`, `vld_query` + `handle_rejection`                                          |
| [`vld-sea`](crates/vld-sea/)            | `crates/vld-sea/`      | [SeaORM](https://www.sea-ql.org/SeaORM/) integration — validate `ActiveModel` before insert/update                                       |
| [`vld-clap`](crates/vld-clap/)          | `crates/vld-clap/`     | [Clap](https://docs.rs/clap) integration — validate CLI arguments via `#[derive(Validate)]`                                             |
| [`vld-ts`](crates/vld-ts/)              | `crates/vld-ts/`       | TypeScript codegen — generates [Zod](https://zod.dev/) schemas from `vld` JSON Schema output                                             |

### vld-derive

Enable with `features = ["derive"]`. Provides `#[derive(Validate)]` with `#[vld(...)]` attributes
on struct fields. Supports `#[serde(rename)]` and `#[serde(rename_all)]` for JSON key mapping.

```rust
use vld::Validate;

#[derive(Debug, Default, serde::Serialize, Validate)]
struct User {
    #[vld(vld::string().min(2).max(50))]
    name: String,
    #[vld(vld::string().email())]
    email: String,
}
```

### vld-axum

Validation extractors for Axum web framework. Automatically validates request data
and returns `422 Unprocessable Entity` with structured JSON errors on failure.

```toml
[dependencies]
vld-axum = "0.1"
```

```rust
use vld_axum::prelude::*;

async fn handler(VldJson(body): VldJson<MySchema>) -> impl IntoResponse {
    // body is already validated
}
```

### vld-actix

Validation extractors for Actix-web framework. Same API surface as `vld-axum`.

```toml
[dependencies]
vld-actix = "0.1"
```

```rust
use vld_actix::prelude::*;

async fn handler(VldJson(body): VldJson<MySchema>) -> impl Responder {
    // body is already validated
}
```

### vld-diesel

Integration with Diesel ORM. Validate data before insert/update, validate rows after load,
and use validated column types (`VldText<S>`, `VldInt<S>`) that enforce constraints at the type level.

```toml
[dependencies]
vld-diesel = { version = "0.1", features = ["sqlite"] }
```

```rust
use vld_diesel::prelude::*;

let validated = validate_insert::<MySchema, _>(&new_row)?;
diesel::insert_into(table).values(&validated.inner).execute(&mut conn)?;
```

### vld-tower

Universal [Tower](https://docs.rs/tower) middleware for JSON body validation.
Works with any Tower-compatible framework (Axum, Hyper, Tonic, Warp).

```toml
[dependencies]
vld-tower = "0.1"
```

```rust
use vld_tower::{ValidateJsonLayer, validated};
use tower::ServiceBuilder;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CreateUser {
        pub name: String  => vld::string().min(2),
        pub email: String => vld::string().email(),
    }
}

// One layer covers all routes
let svc = ServiceBuilder::new()
    .layer(ValidateJsonLayer::<CreateUser>::new())
    .service_fn(handler);

// In handler — zero-cost extraction from extensions
let user: CreateUser = validated(&req);
```

### vld-config

Validate configuration files at load time. Supports [config-rs](https://docs.rs/config)
(TOML, YAML, JSON, ENV) and [figment](https://docs.rs/figment).

```toml
[dependencies]
vld = "0.1"
vld-config = "0.1"  # config-rs by default
# vld-config = { version = "0.1", features = ["figment"] }
```

```rust
use vld_config::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct Settings {
        pub host: String => vld::string().min(1),
        pub port: i64    => vld::number().int().min(1).max(65535),
    }
}

let config = config::Config::builder()
    .add_source(config::File::with_name("config"))
    .add_source(config::Environment::with_prefix("APP"))
    .build().unwrap();

let settings: Settings = from_config(&config).unwrap();
```

### vld-utoipa

Bridge between `vld` and [utoipa](https://docs.rs/utoipa). Define validation once, get
`ToSchema` for free — no need to duplicate schema definitions.

```toml
[dependencies]
vld = { version = "0.1", features = ["openapi"] }
vld-utoipa = "0.1"
utoipa = "5"
```

```rust
use vld::prelude::*;
use vld_utoipa::impl_to_schema;

vld::schema! {
    #[derive(Debug)]
    pub struct CreateUser {
        pub name: String => vld::string().min(2).max(100),
        pub email: String => vld::string().email(),
    }
}

impl_to_schema!(CreateUser);
// Now CreateUser implements utoipa::ToSchema
```

### vld-rocket

[Rocket](https://rocket.rs/) integration with validation extractors and JSON error catchers.

```toml
[dependencies]
vld-rocket = "0.1"
rocket = { version = "0.5", features = ["json"] }
```

```rust
use vld_rocket::prelude::*;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CreateUser {
        pub name: String  => vld::string().min(2),
        pub email: String => vld::string().email(),
    }
}

#[rocket::post("/users", data = "<user>")]
fn create_user(user: VldJson<CreateUser>) -> rocket::serde::json::Json<serde_json::Value> {
    rocket::serde::json::Json(serde_json::json!({"name": user.name}))
}
```

### vld-poem

[Poem](https://docs.rs/poem) integration — extractors `VldJson`, `VldQuery`, `VldForm`.

```toml
[dependencies]
vld-poem = "0.1"
poem = "3"
```

```rust
use poem::handler;
use vld_poem::prelude::*;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CreateUser {
        pub name: String  => vld::string().min(2),
        pub email: String => vld::string().email(),
    }
}

#[handler]
async fn create_user(user: VldJson<CreateUser>) -> poem::web::Json<serde_json::Value> {
    poem::web::Json(serde_json::json!({"name": user.name}))
}
```

### vld-warp

[Warp](https://docs.rs/warp) integration — filters `vld_json`, `vld_query` + `handle_rejection`.

```toml
[dependencies]
vld-warp = "0.1"
warp = "0.3"
```

```rust
use vld_warp::prelude::*;
use warp::Filter;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CreateUser {
        pub name: String  => vld::string().min(2),
        pub email: String => vld::string().email(),
    }
}

let route = warp::post()
    .and(warp::path("users"))
    .and(vld_json::<CreateUser>())
    .map(|u: CreateUser| warp::reply::json(&serde_json::json!({"name": u.name})))
    .recover(handle_rejection);
```

### vld-sea

[SeaORM](https://www.sea-ql.org/SeaORM/) integration — validate `ActiveModel` before insert/update.

```toml
[dependencies]
vld-sea = "0.1"
sea-orm = "1"
```

```rust
use sea_orm::Set;
use vld_sea::prelude::*;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct UserInput {
        pub name: String  => vld::string().min(1).max(100),
        pub email: String => vld::string().email(),
    }
}

let am = user::ActiveModel {
    name: Set("Alice".to_owned()),
    email: Set("alice@example.com".to_owned()),
    ..Default::default()
};

// Validate before insert
vld_sea::validate_active::<UserInput, _>(&am)?;

// Or hook into before_save automatically:
// vld_sea::impl_vld_before_save!(ActiveModel, UserInput);
```

### vld-clap

[Clap](https://docs.rs/clap) integration — validate CLI arguments via `#[derive(Validate)]` directly on the struct.

```toml
[dependencies]
vld-clap = "0.1"
vld = { version = "0.1", features = ["derive"] }
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
```

```rust
use clap::Parser;
use vld::Validate;
use vld_clap::prelude::*;

#[derive(Parser, Debug, serde::Serialize, Validate)]
struct Cli {
    #[arg(long)]
    #[vld(vld::string().email())]
    email: String,
    #[arg(long, default_value_t = 8080)]
    #[vld(vld::number().int().min(1).max(65535))]
    port: i64,
}

fn main() {
    let cli = Cli::parse();
    validate_or_exit(&cli);
    println!("email={}, port={}", cli.email, cli.port);
}
```

### vld-ts

Generate TypeScript [Zod](https://zod.dev/) schemas from JSON Schema output produced by `vld`.
Useful for sharing validation contracts between Rust backend and TypeScript frontend.

```toml
[dependencies]
vld-ts = "0.1"
```

```rust
use vld_ts::generate_zod;

let json_schema = serde_json::json!({
    "type": "object",
    "required": ["name", "email"],
    "properties": {
        "name": { "type": "string", "minLength": 2 },
        "email": { "type": "string", "format": "email" }
    }
});
let ts_code = generate_zod("User", &json_schema);
// Output: export const UserSchema = z.object({ name: z.string().min(2), email: z.string().email() });
```

## License

MIT
