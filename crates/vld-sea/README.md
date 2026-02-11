[![Crates.io](https://img.shields.io/crates/v/vld-sea?style=for-the-badge)](https://crates.io/crates/vld-sea)
[![docs.rs](https://img.shields.io/docsrs/vld-sea?style=for-the-badge)](https://docs.rs/vld-sea)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=for-the-badge)](https://github.com/s00d/vld)
[![GitHub issues](https://img.shields.io/badge/github-issues-orange?style=for-the-badge)](https://github.com/s00d/vld/issues)
[![GitHub stars](https://img.shields.io/badge/github-stars-yellow?style=for-the-badge)](https://github.com/s00d/vld/stargazers)

# vld-sea

[SeaORM](https://www.sea-ql.org/SeaORM/) integration for the **vld** validation library.

Validate `ActiveModel` fields **before** `insert()` / `update()` hits the database.

## Features

- `validate_active` — extract `Set`/`Unchanged` values from `ActiveModel` into JSON and validate
- `validate_model` — validate any `Serialize`-able struct (Model, DTO, etc.)
- `validate_json` — validate raw JSON
- `Validated<S, T>` — wrapper proving data has been validated
- `before_save` — helper for `ActiveModelBehavior::before_save`
- `impl_vld_before_save!` — macro for automatic validation on every insert/update
- `active_model_to_json` — convert ActiveModel to JSON (skips `NotSet` fields)

## Installation

```toml
[dependencies]
vld-sea = "0.1"
vld = "0.1"
sea-orm = "1"
```

## Quick Start

### 1. Define the validation schema

```rust
vld::schema! {
    #[derive(Debug, Clone)]
    pub struct UserInput {
        pub name: String  => vld::string().min(1).max(100),
        pub email: String => vld::string().email(),
    }
}
```

### 2. Validate ActiveModel before insert

```rust
use sea_orm::Set;

let am = user::ActiveModel {
    name: Set("Alice".to_owned()),
    email: Set("alice@example.com".to_owned()),
    ..Default::default()
};

// Validate — returns parsed schema or VldSeaError
vld_sea::validate_active::<UserInput, _>(&am)?;

// Now safe to insert
am.insert(&db).await?;
```

### 3. Automatic validation via `before_save`

**Option A — manual:**

```rust
#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C: ConnectionTrait>(
        self, _db: &C, _insert: bool,
    ) -> Result<Self, DbErr> {
        vld_sea::before_save::<UserInput, _>(&self)?;
        Ok(self)
    }
}
```

**Option B — macro:**

```rust
// Replace the default `impl ActiveModelBehavior for ActiveModel {}`
vld_sea::impl_vld_before_save!(ActiveModel, UserInput);
```

**Option C — separate schemas for insert vs update:**

```rust
vld_sea::impl_vld_before_save!(
    ActiveModel,
    insert: UserInsertSchema,
    update: UserUpdateSchema
);
```

### 4. Validate a DTO

```rust
#[derive(serde::Serialize)]
struct NewUser { name: String, email: String }

let input = NewUser { name: "Bob".into(), email: "bob@example.com".into() };
vld_sea::validate_model::<UserInput, _>(&input)?;
```

## ActiveModel → JSON

`active_model_to_json` converts an ActiveModel to a JSON object. Only `Set` and `Unchanged` fields are included; `NotSet` fields are omitted.

Supported SQL types: `bool`, `i8`–`i64`, `u8`–`u64`, `f32`, `f64`, `String`, `char`.
Feature-gated types (chrono, uuid, etc.) are mapped to `null`.

## Running Examples

```bash
cargo run -p vld-sea --example sea_basic
```

## License

MIT
