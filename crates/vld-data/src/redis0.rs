//! # `vld_data::redis0` — Redis integration for `vld` (redis `>=0.32, <1`)
//!
//! Wrap a connection in a validating wrapper:
//!
//! - [`RedisConn::new(conn)`](RedisConn::new) — sync connections (`redis::ConnectionLike`)
//! - `AsyncRedisConn::new(conn)` — async connections (`redis::aio::ConnectionLike`,
//!   feature `redis-aio`, which needs a redis runtime feature: `redis-aio-tokio`
//!   enables redis' tokio runtime, or enable one on your own `redis` dependency)
//!
//! The wrapper does JSON encode/validate on write and decode/validate on read for:
//!
//! - `set/get`
//! - `mset/mget`
//! - `hset/hget`
//! - `lpush/rpush/lpop/rpop`
//! - `sadd/smembers`
//! - `zadd/zrange`
//! - `publish`
//!
//! Values written are `Serialize + VldParse` (a `vld::schema!` struct with
//! `#[derive(serde::Serialize)]`); values read are parsed with `VldParse`.
//!
//! Feature `redis-0`; the redis crate is re-exported as [`redis`].
//! `vld_data::redis1` is the same module for the other redis major
//! (feature `redis-1`); both can be enabled together.
//!
//! ```no_run
//! use vld_data::redis0::prelude::*;
//! use vld_data::redis0::redis;
#![doc = include_str!("redis/doctest.rs")]
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/messaging/redis>
//!
//! ## Shadowing of native methods
//!
//! The wrapper derefs to the inner connection, so native methods without a
//! validating counterpart (`del`, `expire`, `incr`, …) are called as usual.
//! **The methods listed above shadow the `redis::Commands` /
//! `redis::AsyncCommands` methods of the same name**: `conn.get(k)` always
//! means the validating JSON `get`. To call the native one, go through the trait
//! on the inner connection:
//!
//! ```ignore
//! let raw: Option<String> = redis::Commands::get(&mut *conn, "key")?;
//! ```

/// The redis crate this module is built on.
pub use ::redis0 as redis;

pub use vld;

// One implementation, compiled once per redis major (ARCHITECTURE.md).
#[allow(clippy::duplicate_mod)]
#[path = "redis/shared.rs"]
mod shared;
pub use shared::*;
