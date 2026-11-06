# vld-sqlx

[![crates.io](https://img.shields.io/crates/v/vld-sqlx.svg)](https://crates.io/crates/vld-sqlx)
[![docs.rs](https://docs.rs/vld-sqlx/badge.svg)](https://docs.rs/vld-sqlx)

[SQLx](https://github.com/launchbadge/sqlx) integration for [vld](https://crates.io/crates/vld): validate values before insert / update and after select, and bind validated column wrappers (`VldText`, `VldInt`, `VldFloat`, `VldBool`).

## Install

```toml
[dependencies]
vld = "0.5"
vld-sqlx = "0.5"
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }
```

## Features

| Feature | Default | Effect |
|---|---|---|
| `sqlx-0_8` | yes | SQLx 0.8 |
| `sqlx-0_9` | | SQLx 0.9 (Rust 1.94+) |
| `sqlite` | yes | SQLite driver |
| `postgres` / `mysql` | | Postgres / MySQL drivers |

Enable exactly one of `sqlx-0_8` / `sqlx-0_9`.

## Example

```rust
use vld_sqlx::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct NewUserSchema {
        pub name: String  => vld::string().min(1).max(100),
        pub email: String => vld::string().email(),
    }
}

#[derive(serde::Serialize)]
struct NewUser { name: String, email: String }

let user = NewUser { name: "Alice".into(), email: "alice@example.com".into() };
validate_insert::<NewUserSchema, _>(&user)?;
// sqlx::query("INSERT ...").bind(&user.name).bind(&user.email).execute(&pool).await?;
```

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/database/sqlx>
- API: <https://docs.rs/vld-sqlx>

MSRV: 1.85 (`sqlx-0_9`: 1.94), see [MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md).

## License

MIT
