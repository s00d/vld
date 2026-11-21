# vld-redis

> **Deprecated.** `vld-redis` is not developed any more. Since vld 0.8 it is part of
> [`vld-data`](https://crates.io/crates/vld-data): `vld_data::redis0` / `vld_data::redis1`.
>
> ```toml
> vld-data = { version = "0.8", features = ["redis-0"] }  # or "redis-1"
> ```
>
> Migration guide: <https://s00d.github.io/vld/guides/migration#from-07-to-08>. This version (0.7.1) is 0.7.0 unchanged, with this notice.

[![crates.io](https://img.shields.io/crates/v/vld-redis.svg)](https://crates.io/crates/vld-redis)
[![docs.rs](https://docs.rs/vld-redis/badge.svg)](https://docs.rs/vld-redis)

[Redis](https://crates.io/crates/redis) integration for [vld](https://crates.io/crates/vld): a connection wrapper that validates JSON payloads on write (`set`, `hset`, `lpush`, `publish`, …) and on read (`get`, `hget`, `lpop`, `smembers`, …).

## Install

```toml
[dependencies]
vld = "0.5"
vld-redis = "0.5"
```

The selected `redis` crate is re-exported as `vld_redis::redis`.

## Features

| Feature | Default | Effect |
|---|---|---|
| `redis-0` | yes | `redis` `>=0.32, <1` |
| `redis-1` | | `redis` 1.x |
| `aio` | | `AsyncRedisConn` (async connections) |
| `aio-tokio` | | `aio` plus redis' tokio support |

Enable exactly one of `redis-0` / `redis-1`. `aio` needs a redis runtime feature: use `aio-tokio`, or enable one (`smol-comp`, …) on your own `redis` dependency.

## Example

```rust
use vld_redis::prelude::*;
use vld_redis::redis;

vld::schema! {
    #[derive(Debug, serde::Serialize)]
    pub struct UserSchema {
        pub name: String => vld::string().min(1),
        pub email: String => vld::string().email(),
    }
}

let client = redis::Client::open("redis://127.0.0.1/")?;
let mut conn = RedisConn::new(client.get_connection()?);
conn.set("user:1", &UserSchema { name: "Alice".into(), email: "alice@example.com".into() })?;
let user: Option<UserSchema> = conn.get("user:1")?;
```

The wrapper's methods shadow `redis::Commands` methods of the same name; other commands are available through `Deref`.

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/messaging/redis>
- API: <https://docs.rs/vld-redis>

MSRV: 1.85, see [MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md).

## License

MIT
