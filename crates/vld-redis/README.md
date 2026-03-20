[![Crates.io](https://img.shields.io/crates/v/vld-redis?style=for-the-badge)](https://crates.io/crates/vld-redis)
[![docs.rs](https://img.shields.io/docsrs/vld-redis?style=for-the-badge)](https://docs.rs/vld-redis)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=for-the-badge)](https://github.com/s00d/vld)
[![GitHub issues](https://img.shields.io/badge/github-issues-orange?style=for-the-badge)](https://github.com/s00d/vld/issues)
[![GitHub stars](https://img.shields.io/badge/github-stars-yellow?style=for-the-badge)](https://github.com/s00d/vld/stargazers)

# vld-redis

Redis integration for [`vld`](https://crates.io/crates/vld).

## Overview

`vld-redis` keeps one entrypoint:

- `impl_to_redis!(conn)`

After rebinding, `conn` becomes a validating wrapper:

- built-in auto-conversion methods: `set/get`, `mset/mget`, `hset/hget`, `lpush/rpush/lpop/rpop`, `sadd/smembers`, `zadd/zrange`, `publish`
- all other native Redis methods remain available through deref to inner connection

## Installation

```toml
[dependencies]
vld = { version = "0.2", features = ["serialize"] }
vld-redis = "0.2"
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
let conn = client.get_connection()?;
impl_to_redis!(conn);

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
