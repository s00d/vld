[![Crates.io](https://img.shields.io/crates/v/vld-redis?style=for-the-badge)](https://crates.io/crates/vld-redis)
[![docs.rs](https://img.shields.io/docsrs/vld-redis?style=for-the-badge)](https://docs.rs/vld-redis)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=for-the-badge)](https://github.com/s00d/vld)
[![GitHub issues](https://img.shields.io/badge/github-issues-orange?style=for-the-badge)](https://github.com/s00d/vld/issues)
[![GitHub stars](https://img.shields.io/badge/github-stars-yellow?style=for-the-badge)](https://github.com/s00d/vld/stargazers)

# vld-redis

Redis integration for [`vld`](https://crates.io/crates/vld).

## Version features

- `redis-0` (default, redis 0.32+)
- `redis-1`
- `aio` — async wrapper `AsyncRedisConn` over `redis::aio::ConnectionLike` (no runtime chosen: enable redis' `tokio-comp` / `smol-comp` yourself)
- `aio-tokio` — `aio` plus redis `tokio-comp` (what `aio` enabled before 0.5)

## Overview

Wrap a connection: `let mut conn = RedisConn::new(conn);` (async:
`AsyncRedisConn::new(conn)`). `impl_to_redis!(conn)` / `impl_to_redis_async!(conn)`
only did that rebinding and are deprecated since 0.5.

The wrapper validates:

- built-in auto-conversion methods: `set/get`, `mset/mget`, `hset/hget`, `lpush/rpush/lpop/rpop`, `sadd/smembers`, `zadd/zrange`, `publish`
- all other native Redis methods remain available through deref to inner connection

**Shadowing:** the validating methods above shadow the `redis::Commands` (and
`redis::AsyncCommands`) methods with the same name — `conn.get(k)` is always the
validating JSON `get`. To call the native command, go through the trait on the
inner connection:

```rust
let raw: Option<String> = redis::Commands::get(&mut *conn, "user:1")?;
```

The selected `redis` crate is re-exported as `vld_redis::redis`.

## Async

With the `aio` feature, `AsyncRedisConn::new(conn)` wraps any
`redis::aio::ConnectionLike` (e.g. `MultiplexedConnection`), with the same
methods as `async fn`s:

```rust
let mut conn = AsyncRedisConn::new(client.get_multiplexed_async_connection().await?);
conn.set("user:1", &user).await?;
let loaded: Option<UserSchema> = conn.get("user:1").await?;
```

## Installation

```toml
[dependencies]
vld = "0.5"
vld-redis = "0.5"   # async: features = ["aio-tokio"]
redis = "0.32"
serde = { version = "1", features = ["derive"] }
```

## Quick start

```rust
use redis::Client;
use vld_redis::prelude::*;

vld::schema! {
    #[derive(Debug, serde::Serialize)]
    pub struct UserSchema {
        pub name: String => vld::string().min(1),
        pub email: String => vld::string().email(),
    }
}

let user = UserSchema {
    name: "Alice".into(),
    email: "alice@example.com".into(),
};

let client = Client::open("redis://127.0.0.1/")?;
let mut conn = RedisConn::new(client.get_connection()?);

conn.set("user:1", &user)?;
let loaded: Option<UserSchema> = conn.get("user:1")?;

conn.mset([("user:2", &user), ("user:3", &user)])?;
let _many: Vec<Option<UserSchema>> = conn.mget(["user:2", "user:3"])?;

conn.hset("users:hash", "good", &user)?;
let _from_hash: Option<UserSchema> = conn.hget("users:hash", "good")?;

let _list_len = conn.lpush("users:list", &user)?;
let _set_added = conn.sadd("users:set", &user)?;
let _zadd = conn.zadd("users:zset", 10.0, &user)?;

let _subscribers = conn.publish("users.events", &user)?;
println!("loaded={loaded:?}");
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Example

```bash
cargo run -p vld-redis --example redis_basic
```

## License

MIT
