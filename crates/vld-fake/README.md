[![Crates.io](https://img.shields.io/crates/v/vld-fake?style=for-the-badge)](https://crates.io/crates/vld-fake)
[![docs.rs](https://img.shields.io/docsrs/vld-fake?style=for-the-badge)](https://docs.rs/vld-fake)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=for-the-badge)](https://github.com/s00d/vld)
[![GitHub issues](https://img.shields.io/badge/github-issues-orange?style=for-the-badge)](https://github.com/s00d/vld/issues)
[![GitHub stars](https://img.shields.io/badge/github-stars-yellow?style=for-the-badge)](https://github.com/s00d/vld/stargazers)

# vld-fake

Generate **fake / test data** that satisfies [vld](https://crates.io/crates/vld) validation schemas.

Define your rules once with `vld` and get instant, constraint-aware random data —
perfect for unit tests, property-based testing, seed scripts, and demos.

## Features

| Feature                           | Description                                         |
|-----------------------------------|-----------------------------------------------------|
| `fake_value(schema)`              | Random `serde_json::Value` from a JSON Schema       |
| `fake_json(schema)`               | Pretty-printed JSON string                          |
| `fake_many(schema, n)`            | Generate `n` values at once                         |
| `fake_parsed::<T>(schema)`        | Typed + validated instance via `T::vld_parse_value` |
| `try_fake_parsed::<T>(schema)`    | Same but returns `Result`                           |
| `fake_value_seeded(schema, seed)` | Reproducible output with a fixed seed               |
| `FakeGen::with_rng(rng)`          | Bring your own `Rng`                                |

### Supported JSON Schema features

- **Types**: `string`, `integer`, `number`, `boolean`, `array`, `object`, `null`
- **String formats**: `email`, `uuid`, `url`/`uri`, `ipv4`, `ipv6`, `hostname`, `date`, `time`, `date-time`, `base64`, `cuid2`, `ulid`, `nanoid`, `emoji`, `phone`, `credit-card`, `mac-address`, `hex-color`, `semver`, `slug`
- **Constraints**: `minLength`/`maxLength`, `minimum`/`maximum`, `exclusiveMinimum`/`exclusiveMaximum`, `multipleOf`, `minItems`/`maxItems`, `uniqueItems`
- **Combinators**: `enum`, `const`, `oneOf`, `anyOf`, `allOf`, `prefixItems` (tuples)
- **Objects**: `properties`, `required`, `additionalProperties`

### Smart field-name heuristics

When generating object properties, field names are used to infer realistic data **even without** an explicit `format`:

| Field name pattern              | Generated data                              |
|---------------------------------|---------------------------------------------|
| `name`, `full_name`, `author`   | Realistic full name ("Alice Johnson")       |
| `first_name`                    | First name from dictionary                  |
| `last_name`, `surname`          | Last name from dictionary                   |
| `username`, `login`, `handle`   | Lowercase name + digits ("alice42")         |
| `email`, `mail`                 | Realistic email ("alice.johnson@gmail.com") |
| `phone`, `mobile`, `tel`        | Phone number ("+1 (555) 123-4567")          |
| `city`, `town`                  | Real city name                              |
| `country`                       | Real country name                           |
| `state`, `province`             | US state name                               |
| `street`, `address`             | Street address ("1234 Oak Avenue")          |
| `zip`, `postal_code`            | 5-digit zip code                            |
| `company`, `organization`       | Company name                                |
| `department`                    | Department name                             |
| `job_title`, `position`         | Job title                                   |
| `url`, `website`, `link`        | Realistic URL                               |
| `domain`, `hostname`            | Domain name                                 |
| `id`, `uuid`, `guid`            | UUID v4                                     |
| `password`, `secret`            | Strong password (10-18 chars, mixed)        |
| `token`, `api_key`              | Random alphanumeric token                   |
| `description`, `bio`, `summary` | Lorem-style sentence                        |
| `product_name`, `item`          | "Adjective Noun" product name               |
| `sku`, `product_code`           | SKU code ("ABC-12345")                      |
| `category`, `genre`             | Category from dictionary                    |
| `tag`, `label`                  | Tag from dictionary                         |
| `color`, `colour`               | Color name                                  |
| `hex_color`                     | Hex color ("#a3f2c1")                       |
| `currency`                      | ISO currency code                           |
| `locale`, `lang`                | Locale code ("en-US")                       |
| `timezone`, `tz`                | IANA timezone                               |
| `mime_type`, `content_type`     | MIME type                                   |
| `file_name`                     | File name with extension                    |
| `version`, `semver`             | Semver string ("1.4.12")                    |
| `credit_card`                   | Luhn-valid card number                      |
| `isbn`                          | Valid ISBN-13                               |
| `latitude`, `longitude`         | Geo coordinates                             |
| `user_agent`                    | Realistic browser UA string                 |
| `mac_address`                   | MAC address                                 |

### Dictionaries

The crate ships with built-in dictionaries (~2000 entries total):

- **200** first names, **130** last names
- **95** cities, **50** countries, **50** US states
- **48** street names, **20** street suffixes
- **40** companies, **22** departments, **39** job titles
- **35** adjectives, **38** product nouns, **26** categories, **27** tags
- **15** email domains, **24** TLDs
- **20** MIME types, **44** file extensions
- **39** colors, **27** currencies, **35** locales, **32** timezones
- **60** emojis, **80** words, **100** lorem words

## Installation

```toml
[dependencies]
vld = { version = "0.1", features = ["openapi"] }
vld-fake = "0.1"
```

## Quick Start

```rust
use vld::prelude::*;
use vld_fake::prelude::*;

vld::schema! {
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct User {
        pub name:  String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
        pub age:   i64    => vld::number().int().min(18).max(99),
    }
}

// One line — enables User::fake(), User::fake_many(), User::fake_seeded()
vld_fake::impl_fake!(User);

fn main() {
    // Typed access — user.name, user.email, user.age
    let user = User::fake();
    println!("{} <{}> age={}", user.name, user.email, user.age);

    // Multiple at once
    let users = User::fake_many(5);
    for u in &users {
        println!("  {} <{}>", u.name, u.email);
    }

    // Reproducible — same seed → same data
    let u1 = User::fake_seeded(42);
    let u2 = User::fake_seeded(42);
    assert_eq!(u1.name, u2.name);

    // Fallible variant
    let result = User::try_fake();
    assert!(result.is_ok());
}
```

### Low-level (untyped) API

If you need raw `serde_json::Value` or work with arbitrary JSON Schemas:

```rust
let schema = User::json_schema();
let value = fake_value(&schema);        // -> serde_json::Value
let json  = fake_json(&schema);         // -> String
let many  = fake_many(&schema, 10);     // -> Vec<Value>
let seeded = fake_value_seeded(&schema, 42);
```

## Nested Schemas

```rust
vld::schema! {
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct Address {
        pub city: String => vld::string().min(1),
        pub zip:  String => vld::string().min(5).max(10),
    }
}

vld::schema! {
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct UserWithAddress {
        pub name:    String  => vld::string().min(2),
        pub address: Address => vld::nested(Address::parse_value),
    }
}

let schema = UserWithAddress::json_schema();
let value = fake_value(&schema);
// { "name": "kRtBwZ", "address": { "city": "aBcDe", "zip": "12345" } }
```

## Raw JSON Schema

You don't need `vld::schema!` — any valid JSON Schema works:

```rust
let schema = serde_json::json!({
    "type": "object",
    "required": ["host", "port"],
    "properties": {
        "host": { "type": "string", "format": "ipv4" },
        "port": { "type": "integer", "minimum": 1, "maximum": 65535 }
    }
});
let config = vld_fake::fake_value(&schema);
```

## Custom RNG

```rust
use rand::SeedableRng;
use vld_fake::FakeGen;

let rng = rand::rngs::StdRng::seed_from_u64(12345);
let mut gen = FakeGen::with_rng(rng);
let value = gen.value(&schema);
```

## Use Cases

- **Unit tests** — generate valid fixtures without manual JSON
- **Property-based testing** — combine with `proptest` strategies
- **Seed / migration scripts** — fill databases with realistic data
- **API mocking** — respond with schema-conforming payloads
- **Documentation** — auto-generate example request/response bodies

## Running the Example

```bash
cargo run -p vld-fake --example fake_basic
```

## License

MIT
