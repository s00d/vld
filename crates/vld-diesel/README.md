[![Crates.io](https://img.shields.io/crates/v/vld-diesel?style=for-the-badge)](https://crates.io/crates/vld-diesel)
[![docs.rs](https://img.shields.io/docsrs/vld-diesel?style=for-the-badge)](https://docs.rs/vld-diesel)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=for-the-badge)](https://github.com/s00d/vld)
[![GitHub issues](https://img.shields.io/badge/github-issues-orange?style=for-the-badge)](https://github.com/s00d/vld/issues)
[![GitHub stars](https://img.shields.io/badge/github-stars-yellow?style=for-the-badge)](https://github.com/s00d/vld/stargazers)

# vld-diesel

Diesel integration for the [vld](https://crates.io/crates/vld) validation library.

Validate data **before** inserting into the database. Provides:

| Feature | Description |
|---------|-------------|
| `validate_insert::<Schema, _>(&value)` | Standalone validation before INSERT |
| `validate_update::<Schema, _>(&value)` | Standalone validation before UPDATE |
| `validate_row::<Schema, _>(&row)` | Validate data loaded from DB |
| `Validated<Schema, T>` | Wrapper that guarantees `T` passes `Schema` |
| `VldText<S>` | Validated `String` column type with Diesel `ToSql`/`FromSql` |
| `VldInt<S>` | Validated `i64` column type with Diesel `ToSql`/`FromSql` |

## Installation

```toml
[dependencies]
vld-diesel = "0.1"
vld = { version = "0.1", features = ["serialize"] }
diesel = { version = "2", features = ["sqlite"] }
```

### Backend features

| Feature | Backend |
|---------|---------|
| `sqlite` (default) | SQLite |
| `postgres` | PostgreSQL |
| `mysql` | MySQL |

## Quick start

### 1. Validate before insert

```rust
use vld_diesel::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct NewUserSchema {
        pub name: String  => vld::string().min(1).max(100),
        pub email: String => vld::string().email(),
        pub age: i64      => vld::number().int().min(0).max(150),
    }
}

#[derive(Insertable, serde::Serialize)]
#[diesel(table_name = users)]
struct NewUser { name: String, email: String, age: i64 }

// Validate
let user = NewUser { name: "Alice".into(), email: "alice@example.com".into(), age: 30 };
validate_insert::<NewUserSchema, _>(&user)?;

// Then insert
diesel::insert_into(users::table).values(&user).execute(&mut conn)?;
```

### 2. Validated wrapper

```rust
let user = NewUser { name: "Bob".into(), email: "bob@example.com".into(), age: 25 };
let validated = Validated::<NewUserSchema, _>::new(user)?;

// The inner value is guaranteed valid
diesel::insert_into(users::table)
    .values(validated.inner())
    .execute(&mut conn)?;
```

### 3. Typed columns

```rust
use vld_diesel::VldText;

vld::schema! {
    #[derive(Debug)]
    pub struct EmailField {
        pub value: String => vld::string().email(),
    }
}

// Validates on construction
let email = VldText::<EmailField>::new("user@example.com")?;

// Use in Diesel models — implements ToSql/FromSql for Text
```

### 4. Validate rows from DB

```rust
let users: Vec<User> = users::table.load(&mut conn)?;
for user in &users {
    validate_row::<NewUserSchema, _>(user)?;
}
```

## Running the example

```bash
cargo run -p vld-diesel --example diesel_basic
```

## License

MIT
