# vld-surrealdb

[![crates.io](https://img.shields.io/crates/v/vld-surrealdb.svg)](https://crates.io/crates/vld-surrealdb)
[![docs.rs](https://docs.rs/vld-surrealdb/badge.svg)](https://docs.rs/vld-surrealdb)

[SurrealDB](https://surrealdb.com) helpers for [vld](https://crates.io/crates/vld): validate JSON documents before create / insert / merge and after select. SDK-agnostic: no `surrealdb` dependency, works through `serde`.

## Install

```toml
[dependencies]
vld = "0.5"
vld-surrealdb = "0.5"
```

## Example

```rust
use vld_surrealdb::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct PersonSchema {
        pub name: String  => vld::string().min(1).max(100),
        pub email: String => vld::string().email(),
    }
}

#[derive(serde::Serialize)]
struct Person { name: String, email: String }

let person = Person { name: "Alice".into(), email: "alice@example.com".into() };
validate_content::<PersonSchema, _>(&person)?;
// db.create("person").content(person).await?;
```

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/database/surrealdb>
- API: <https://docs.rs/vld-surrealdb>

MSRV: 1.85, see [MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md).

## License

MIT
