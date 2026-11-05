//! # vld-redis
//!
//! Redis integration for `vld`.
//!
//! ## Overview
//!
//! Wrap a connection in a validating wrapper:
//!
//! - [`RedisConn::new(conn)`](RedisConn::new) — sync connections (`redis::ConnectionLike`)
//! - `AsyncRedisConn::new(conn)` — async connections (`redis::aio::ConnectionLike`,
//!   feature `aio`; `aio-tokio` also enables redis' tokio runtime)
//!
//! (`impl_to_redis!(conn)` / `impl_to_redis_async!(conn)` only did
//! `let mut conn = RedisConn::new(conn);` and are deprecated.)
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

use std::fmt;
use std::ops::{Deref, DerefMut};

#[cfg(all(feature = "redis-0", feature = "redis-1"))]
compile_error!("Enable exactly one Redis version feature: `redis-0` or `redis-1`.");
#[cfg(not(any(feature = "redis-0", feature = "redis-1")))]
compile_error!("Enable one Redis version feature: `redis-0` or `redis-1`.");

/// The `redis` crate version selected by the `redis-0` / `redis-1` feature.
#[cfg(feature = "redis-0")]
pub use ::redis;
/// The `redis` crate version selected by the `redis-0` / `redis-1` feature.
#[cfg(feature = "redis-1")]
pub use redis1 as redis;

pub use vld;

fn encode<V>(value: &V) -> Result<Vec<u8>, VldRedisError>
where
    V: serde::Serialize + vld::schema::VldParse,
{
    let json =
        serde_json::to_value(value).map_err(|e| VldRedisError::Serialization(e.to_string()))?;
    V::vld_parse_value(&json).map_err(VldRedisError::Validation)?;
    serde_json::to_vec(&json).map_err(|e| VldRedisError::Serialization(e.to_string()))
}

fn decode<T: vld::schema::VldParse>(bytes: &[u8]) -> Result<T, VldRedisError> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| VldRedisError::Deserialization(e.to_string()))?;
    T::vld_parse_value(&value).map_err(VldRedisError::Validation)
}

fn decode_opt<T: vld::schema::VldParse>(raw: Option<Vec<u8>>) -> Result<Option<T>, VldRedisError> {
    raw.map(|b| decode(&b)).transpose()
}

fn decode_all<T: vld::schema::VldParse>(raw: Vec<Vec<u8>>) -> Result<Vec<T>, VldRedisError> {
    raw.iter().map(|b| decode(b)).collect()
}

macro_rules! wrapper {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        pub struct $name<C> {
            inner: C,
        }

        impl<C> $name<C> {
            pub fn new(inner: C) -> Self {
                Self { inner }
            }

            pub fn into_inner(self) -> C {
                self.inner
            }
        }

        impl<C> Deref for $name<C> {
            type Target = C;

            fn deref(&self) -> &Self::Target {
                &self.inner
            }
        }

        impl<C> DerefMut for $name<C> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.inner
            }
        }
    };
}

wrapper!(
    /// Sync Redis connection wrapper with validate+JSON behavior.
    ///
    /// Its methods shadow `redis::Commands` methods of the same name; see the
    /// crate docs for calling the native ones.
    RedisConn
);

#[cfg(feature = "aio")]
wrapper!(
    /// Async Redis connection wrapper with validate+JSON behavior (feature `aio`).
    ///
    /// Same methods as [`RedisConn`], but `async`. They shadow
    /// `redis::AsyncCommands` methods of the same name.
    AsyncRedisConn
);

/// Defines every validating command once, for both the sync and async wrapper.
/// Each command builds a `redis::Cmd` (`build`), runs it into `raw`, then maps it.
macro_rules! commands {
    ($(
        fn $name:ident<$($gen:tt),*>($($arg:ident: $ty:ty),*) -> $ret:ty
        where [$($bound:tt)*]
        { build: $build:expr, raw: $raw:ty, map: $map:expr $(,)? }
    )*) => {
        impl<C: redis::ConnectionLike> RedisConn<C> {
            $(
                pub fn $name<$($gen),*>(&mut self, $($arg: $ty),*) -> Result<$ret, VldRedisError>
                where $($bound)*
                {
                    let cmd: redis::Cmd = $build;
                    let raw: $raw = cmd.query(&mut self.inner)?;
                    ($map)(raw)
                }
            )*
        }

        #[cfg(feature = "aio")]
        impl<C: redis::aio::ConnectionLike> AsyncRedisConn<C> {
            $(
                pub async fn $name<$($gen),*>(&mut self, $($arg: $ty),*) -> Result<$ret, VldRedisError>
                where $($bound)*
                {
                    let cmd: redis::Cmd = $build;
                    let raw: $raw = cmd.query_async(&mut self.inner).await?;
                    ($map)(raw)
                }
            )*
        }
    };
}

fn cmd1<A: redis::ToRedisArgs>(name: &str, a: A) -> redis::Cmd {
    let mut c = redis::cmd(name);
    c.arg(a);
    c
}

commands! {
    fn set<K, V>(key: K, value: &V) -> ()
    where [K: redis::ToRedisArgs, V: serde::Serialize + vld::schema::VldParse]
    { build: { let mut c = cmd1("SET", key); c.arg(encode(value)?); c }, raw: (), map: Ok }

    fn get<K, T>(key: K) -> Option<T>
    where [K: redis::ToRedisArgs, T: vld::schema::VldParse]
    { build: cmd1("GET", key), raw: Option<Vec<u8>>, map: decode_opt }

    fn mset<'a, K, V, I>(items: I) -> ()
    where [I: IntoIterator<Item = (K, &'a V)>, K: redis::ToRedisArgs, V: serde::Serialize + vld::schema::VldParse + 'a]
    {
        build: {
            let mut c = redis::cmd("MSET");
            for (key, value) in items {
                c.arg(key).arg(encode(value)?);
            }
            c
        },
        raw: (),
        map: Ok,
    }

    fn mget<K, T, I>(keys: I) -> Vec<Option<T>>
    where [I: IntoIterator<Item = K>, K: redis::ToRedisArgs, T: vld::schema::VldParse]
    {
        build: {
            let mut c = redis::cmd("MGET");
            for key in keys {
                c.arg(key);
            }
            c
        },
        raw: Vec<Option<Vec<u8>>>,
        map: |raw: Vec<Option<Vec<u8>>>| raw.into_iter().map(decode_opt).collect(),
    }

    fn hset<K, F, V>(key: K, field: F, value: &V) -> ()
    where [K: redis::ToRedisArgs, F: redis::ToRedisArgs, V: serde::Serialize + vld::schema::VldParse]
    { build: { let mut c = cmd1("HSET", key); c.arg(field).arg(encode(value)?); c }, raw: (), map: Ok }

    fn hget<K, F, T>(key: K, field: F) -> Option<T>
    where [K: redis::ToRedisArgs, F: redis::ToRedisArgs, T: vld::schema::VldParse]
    { build: { let mut c = cmd1("HGET", key); c.arg(field); c }, raw: Option<Vec<u8>>, map: decode_opt }

    fn lpush<K, V>(key: K, value: &V) -> usize
    where [K: redis::ToRedisArgs, V: serde::Serialize + vld::schema::VldParse]
    { build: { let mut c = cmd1("LPUSH", key); c.arg(encode(value)?); c }, raw: usize, map: Ok }

    fn rpush<K, V>(key: K, value: &V) -> usize
    where [K: redis::ToRedisArgs, V: serde::Serialize + vld::schema::VldParse]
    { build: { let mut c = cmd1("RPUSH", key); c.arg(encode(value)?); c }, raw: usize, map: Ok }

    fn lpop<K, T>(key: K) -> Option<T>
    where [K: redis::ToRedisArgs, T: vld::schema::VldParse]
    { build: cmd1("LPOP", key), raw: Option<Vec<u8>>, map: decode_opt }

    fn rpop<K, T>(key: K) -> Option<T>
    where [K: redis::ToRedisArgs, T: vld::schema::VldParse]
    { build: cmd1("RPOP", key), raw: Option<Vec<u8>>, map: decode_opt }

    fn sadd<K, V>(key: K, value: &V) -> usize
    where [K: redis::ToRedisArgs, V: serde::Serialize + vld::schema::VldParse]
    { build: { let mut c = cmd1("SADD", key); c.arg(encode(value)?); c }, raw: usize, map: Ok }

    fn smembers<K, T>(key: K) -> Vec<T>
    where [K: redis::ToRedisArgs, T: vld::schema::VldParse]
    { build: cmd1("SMEMBERS", key), raw: Vec<Vec<u8>>, map: decode_all }

    fn zadd<K, V>(key: K, score: f64, value: &V) -> usize
    where [K: redis::ToRedisArgs, V: serde::Serialize + vld::schema::VldParse]
    { build: { let mut c = cmd1("ZADD", key); c.arg(score).arg(encode(value)?); c }, raw: usize, map: Ok }

    fn zrange<K, T>(key: K, start: isize, stop: isize) -> Vec<T>
    where [K: redis::ToRedisArgs, T: vld::schema::VldParse]
    { build: { let mut c = cmd1("ZRANGE", key); c.arg(start).arg(stop); c }, raw: Vec<Vec<u8>>, map: decode_all }

    fn publish<Cn, V>(channel: Cn, value: &V) -> i64
    where [Cn: redis::ToRedisArgs, V: serde::Serialize + vld::schema::VldParse]
    { build: { let mut c = cmd1("PUBLISH", channel); c.arg(encode(value)?); c }, raw: i64, map: Ok }
}

#[derive(Debug)]
pub enum VldRedisError {
    Validation(vld::error::VldError),
    Serialization(String),
    Deserialization(String),
    Redis(redis::RedisError),
}

impl fmt::Display for VldRedisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VldRedisError::Validation(e) => write!(f, "Validation error: {e}"),
            VldRedisError::Serialization(e) => write!(f, "Serialization error: {e}"),
            VldRedisError::Deserialization(e) => write!(f, "Deserialization error: {e}"),
            VldRedisError::Redis(e) => write!(f, "Redis error: {e}"),
        }
    }
}

impl std::error::Error for VldRedisError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VldRedisError::Validation(e) => Some(e),
            VldRedisError::Redis(e) => Some(e),
            VldRedisError::Serialization(_) | VldRedisError::Deserialization(_) => None,
        }
    }
}

impl From<vld::error::VldError> for VldRedisError {
    fn from(value: vld::error::VldError) -> Self {
        Self::Validation(value)
    }
}

impl From<redis::RedisError> for VldRedisError {
    fn from(value: redis::RedisError) -> Self {
        Self::Redis(value)
    }
}

/// Rebind a sync Redis connection into a `vld`-aware [`RedisConn`].
///
/// Deprecated: it only shadows the variable — write
/// `let mut conn = RedisConn::new(conn);`.
#[deprecated(since = "0.5.0", note = "use `let mut conn = RedisConn::new(conn);`")]
#[macro_export]
macro_rules! impl_to_redis {
    ($conn:ident) => {
        let mut $conn = $crate::RedisConn::new($conn);
    };
}

/// Rebind an async Redis connection into a `vld`-aware [`AsyncRedisConn`]
/// (feature `aio`).
///
/// Deprecated: it only shadows the variable — write
/// `let mut conn = AsyncRedisConn::new(conn);`.
#[cfg(feature = "aio")]
#[deprecated(
    since = "0.5.0",
    note = "use `let mut conn = AsyncRedisConn::new(conn);`"
)]
#[macro_export]
macro_rules! impl_to_redis_async {
    ($conn:ident) => {
        let mut $conn = $crate::AsyncRedisConn::new($conn);
    };
}

pub mod prelude {
    #[allow(deprecated)]
    pub use crate::impl_to_redis;
    #[cfg(feature = "aio")]
    #[allow(deprecated)]
    pub use crate::impl_to_redis_async;
    #[cfg(feature = "aio")]
    pub use crate::AsyncRedisConn;
    pub use crate::{RedisConn, VldRedisError};
    pub use vld::prelude::*;
}
