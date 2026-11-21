# vld-diesel

> **Deprecated.** `vld-diesel` is not developed any more. Since vld 0.8 it is part of
> [`vld-data`](https://crates.io/crates/vld-data): `vld_data::diesel`.
>
> ```toml
> vld-data = { version = "0.8", features = ["diesel", "sqlite"] }
> ```
>
> Migration guide: <https://s00d.github.io/vld/guides/migration#from-07-to-08>. This version (0.7.1) is 0.7.0 unchanged, with this notice.

[![crates.io](https://img.shields.io/crates/v/vld-diesel.svg)](https://crates.io/crates/vld-diesel)
[![docs.rs](https://docs.rs/vld-diesel/badge.svg)](https://docs.rs/vld-diesel)

[Diesel](https://diesel.rs) integration for [vld](https://crates.io/crates/vld): validate `Insertable` structs before insert / update and use validated column types (`VldText`, `VldInt`, `VldFloat`, `VldBool`) in models and filters.

## Install

```toml
[dependencies]
vld = "0.5"
vld-diesel = "0.5"
diesel = { version = "2", features = ["sqlite"] }
```

## Features

| Feature | Default | Effect |
|---|---|---|
| `sqlite` | yes | SQLite backend |
| `postgres` | | Postgres backend |
| `mysql` | | MySQL backend |

## Example

```rust
use vld_diesel::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct NewUserSchema {
        pub name: String  => vld::string().min(1).max(100),
        pub email: String => vld::string().email(),
    }
}

let validated = Validated::<NewUserSchema, _>::new(new_user)?;
diesel::insert_into(users::table)
    .values(validated.inner())
    .execute(&mut conn)?;
```

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/database/diesel>
- API: <https://docs.rs/vld-diesel>

MSRV: 1.85, see [MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md).

## License

MIT
