# vld-sqlx

SQLx integration for the [vld](https://crates.io/crates/vld) validation library.

Validate data **before** inserting into the database. Provides:

| Feature | Description |
|---------|-------------|
| `validate_insert::<Schema, _>(&value)` | Standalone validation before INSERT |
| `validate_update::<Schema, _>(&value)` | Standalone validation before UPDATE |
| `validate_row::<Schema, _>(&row)` | Validate data loaded from DB |
| `validate_rows::<Schema, _>(&rows)` | Batch validate with row index on error |
| `Validated<Schema, T>` | Wrapper that guarantees `T` passes `Schema` |
| `VldText<S>` | Validated `String` column — SQLx `Type`/`Encode`/`Decode` |
| `VldInt<S>` | Validated `i64` column — SQLx `Type`/`Encode`/`Decode` |
| `VldFloat<S>` | Validated `f64` column — SQLx `Type`/`Encode`/`Decode` |
| `VldBool<S>` | Validated `bool` column — SQLx `Type`/`Encode`/`Decode` |

## Installation

```toml
[dependencies]
vld-sqlx = "0.1"
vld = { version = "0.1", features = ["serialize"] }
sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio"] }
```

### Backend features

| Feature | Backend |
|---------|---------|
| `sqlite` (default) | SQLite |
| `postgres` | PostgreSQL |
| `mysql` | MySQL |

Generic SQLx trait implementations (`Type`, `Encode`, `Decode`) work with any backend
where the underlying primitive type (`String`, `i64`, `f64`, `bool`) is supported.

## Quick Start

### 1. Validate before insert

```rust
use vld_sqlx::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct NewUserSchema {
        pub name: String  => vld::string().min(1).max(100),
        pub email: String => vld::string().email(),
        pub age: i64      => vld::number().int().min(0).max(150),
    }
}

#[derive(serde::Serialize)]
struct NewUser { name: String, email: String, age: i64 }

let user = NewUser { name: "Alice".into(), email: "alice@example.com".into(), age: 30 };
validate_insert::<NewUserSchema, _>(&user)?;

// Then insert via sqlx
sqlx::query("INSERT INTO users (name, email, age) VALUES (?, ?, ?)")
    .bind(&user.name)
    .bind(&user.email)
    .bind(user.age)
    .execute(&pool)
    .await?;
```

### 2. Validated wrapper

```rust
let user = NewUser { name: "Bob".into(), email: "bob@example.com".into(), age: 25 };
let validated = Validated::<NewUserSchema, _>::new(user)?;

// The inner value is guaranteed valid
sqlx::query("INSERT INTO users (name, email, age) VALUES (?, ?, ?)")
    .bind(&validated.inner().name)
    .bind(&validated.inner().email)
    .bind(validated.inner().age)
    .execute(&pool)
    .await?;
```

### 3. Typed columns

Use `VldText`, `VldInt`, `VldFloat`, `VldBool` as column types in your models.
They implement SQLx `Type`, `Encode`, and `Decode` for seamless DB integration.

```rust
use vld_sqlx::{VldText, VldInt};

vld::schema! {
    #[derive(Debug)]
    pub struct EmailField {
        pub value: String => vld::string().email(),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct AgeField {
        pub value: i64 => vld::number().int().min(0).max(150),
    }
}

// Validates on construction
let email = VldText::<EmailField>::new("user@example.com")?;
let age = VldInt::<AgeField>::new(25)?;

// Bind directly in queries
sqlx::query("INSERT INTO users (email, age) VALUES (?, ?)")
    .bind(&email)
    .bind(&age)
    .execute(&pool)
    .await?;

// Decode from query results
let row = sqlx::query("SELECT email, age FROM users LIMIT 1")
    .fetch_one(&pool)
    .await?;
let decoded_email: VldText<EmailField> = row.get("email");
let decoded_age: VldInt<AgeField> = row.get("age");
```

Works with `#[derive(sqlx::FromRow)]` too:

```rust
#[derive(sqlx::FromRow)]
struct User {
    id: i64,
    email: VldText<EmailField>,
    age: VldInt<AgeField>,
}

let user: User = sqlx::query_as("SELECT id, email, age FROM users LIMIT 1")
    .fetch_one(&pool)
    .await?;
```

### 4. Batch validation

```rust
let rows: Vec<NewUser> = load_rows();
match vld_sqlx::validate_rows::<NewUserSchema, _>(&rows) {
    Ok(()) => println!("All rows valid"),
    Err((index, error)) => println!("Row {} invalid: {}", index, error),
}
```

### 5. Error conversion to sqlx::Error

`VldSqlxError` implements `Into<sqlx::Error>`, so you can use `?` in functions
returning `Result<T, sqlx::Error>`:

```rust
async fn insert_user(pool: &SqlitePool, user: &NewUser) -> Result<(), sqlx::Error> {
    vld_sqlx::validate_insert::<NewUserSchema, _>(user)?; // auto-converts to sqlx::Error
    sqlx::query("INSERT INTO users (name, email, age) VALUES (?, ?, ?)")
        .bind(&user.name)
        .bind(&user.email)
        .bind(user.age)
        .execute(pool)
        .await?;
    Ok(())
}
```

## Comparison with vld-diesel

| | vld-sqlx | vld-diesel |
|---|---|---|
| ORM | SQLx (async, compile-time checks) | Diesel (sync, schema DSL) |
| Column types | `VldText`, `VldInt`, `VldFloat`, `VldBool` | `VldText`, `VldInt` |
| Batch validation | `validate_rows()` with row index | — |
| Error conversion | `Into<sqlx::Error>` | — |
| `FromRow` compat | `#[derive(sqlx::FromRow)]` works | `QueryableByName` works |

## Running the example

```bash
cargo run -p vld-sqlx --example sqlx_basic
```

## License

MIT
