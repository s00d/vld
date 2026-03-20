//! # vld-redis
//!
//! Redis integration for `vld`.
//!
//! ## Overview
//!
//! `vld-redis` keeps one entrypoint macro:
//!
//! - `impl_to_redis!(conn)`
//!
//! After rebinding, `conn` becomes a validating wrapper with auto conversion for:
//!
//! - `set/get`
//! - `mset/mget`
//! - `hset/hget`
//! - `lpush/rpush/lpop/rpop`
//! - `sadd/smembers`
//! - `zadd/zrange`
//! - `publish`
//!
//! All other native Redis methods are still available through deref to the inner connection.

use std::fmt;
use std::ops::{Deref, DerefMut};

pub use vld;

/// Redis connection wrapper with validate+JSON behavior.
pub struct RedisConn<C> {
    inner: C,
}

impl<C> RedisConn<C> {
    pub fn new(inner: C) -> Self {
        Self { inner }
    }

    pub fn into_inner(self) -> C {
        self.inner
    }
}

impl<C> Deref for RedisConn<C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<C> DerefMut for RedisConn<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<C> RedisConn<C>
where
    C: redis::ConnectionLike,
{
    fn encode_json_value<V>(value: &V) -> Result<serde_json::Value, VldRedisError>
    where
        V: serde::Serialize + vld::schema::VldParse + ?Sized,
    {
        let json =
            serde_json::to_value(value).map_err(|e| VldRedisError::Serialization(e.to_string()))?;
        <V as vld::schema::VldParse>::vld_parse_value(&json).map_err(VldRedisError::Validation)?;
        Ok(json)
    }

    fn encode_json_string<V>(value: &V) -> Result<String, VldRedisError>
    where
        V: serde::Serialize + vld::schema::VldParse + ?Sized,
    {
        let json = Self::encode_json_value(value)?;
        serde_json::to_string(&json).map_err(|e| VldRedisError::Serialization(e.to_string()))
    }

    fn encode_json_bytes<V>(value: &V) -> Result<Vec<u8>, VldRedisError>
    where
        V: serde::Serialize + vld::schema::VldParse + ?Sized,
    {
        let json = Self::encode_json_value(value)?;
        serde_json::to_vec(&json).map_err(|e| VldRedisError::Serialization(e.to_string()))
    }

    fn decode_json_bytes<T>(bytes: &[u8]) -> Result<T, VldRedisError>
    where
        T: vld::schema::VldParse,
    {
        let value: serde_json::Value = serde_json::from_slice(bytes)
            .map_err(|e| VldRedisError::Deserialization(e.to_string()))?;
        <T as vld::schema::VldParse>::vld_parse_value(&value).map_err(VldRedisError::Validation)
    }

    pub fn set<K, V>(&mut self, key: K, value: &V) -> Result<(), VldRedisError>
    where
        K: redis::ToRedisArgs,
        V: serde::Serialize + vld::schema::VldParse + ?Sized,
    {
        let s = Self::encode_json_string(value)?;
        redis::cmd("SET")
            .arg(key)
            .arg(s)
            .query::<()>(&mut self.inner)?;
        Ok(())
    }

    pub fn get<K, T>(&mut self, key: K) -> Result<Option<T>, VldRedisError>
    where
        K: redis::ToRedisArgs,
        T: vld::schema::VldParse,
    {
        let raw: Option<Vec<u8>> = redis::cmd("GET").arg(key).query(&mut self.inner)?;
        raw.map(|bytes| Self::decode_json_bytes(&bytes))
        .transpose()
    }

    pub fn mset<'a, K, V, I>(&mut self, items: I) -> Result<(), VldRedisError>
    where
        I: IntoIterator<Item = (K, &'a V)>,
        K: redis::ToRedisArgs,
        V: serde::Serialize + vld::schema::VldParse + ?Sized + 'a,
    {
        let mut cmd = redis::cmd("MSET");
        for (key, value) in items {
            let encoded = Self::encode_json_string(value)?;
            cmd.arg(key).arg(encoded);
        }
        cmd.query::<()>(&mut self.inner)?;
        Ok(())
    }

    pub fn mget<K, T, I>(&mut self, keys: I) -> Result<Vec<Option<T>>, VldRedisError>
    where
        I: IntoIterator<Item = K>,
        K: redis::ToRedisArgs,
        T: vld::schema::VldParse,
    {
        let mut cmd = redis::cmd("MGET");
        for key in keys {
            cmd.arg(key);
        }
        let raw: Vec<Option<Vec<u8>>> = cmd.query(&mut self.inner)?;
        raw.into_iter()
            .map(|opt| opt.map(|bytes| Self::decode_json_bytes(&bytes)).transpose())
            .collect()
    }

    pub fn hset<K, F, V>(&mut self, key: K, field: F, value: &V) -> Result<(), VldRedisError>
    where
        K: redis::ToRedisArgs,
        F: redis::ToRedisArgs,
        V: serde::Serialize + vld::schema::VldParse + ?Sized,
    {
        let s = Self::encode_json_string(value)?;
        redis::cmd("HSET")
            .arg(key)
            .arg(field)
            .arg(s)
            .query::<()>(&mut self.inner)?;
        Ok(())
    }

    pub fn hget<K, F, T>(&mut self, key: K, field: F) -> Result<Option<T>, VldRedisError>
    where
        K: redis::ToRedisArgs,
        F: redis::ToRedisArgs,
        T: vld::schema::VldParse,
    {
        let raw: Option<Vec<u8>> = redis::cmd("HGET")
            .arg(key)
            .arg(field)
            .query(&mut self.inner)?;
        raw.map(|bytes| Self::decode_json_bytes(&bytes))
        .transpose()
    }

    pub fn lpush<K, V>(&mut self, key: K, value: &V) -> Result<usize, VldRedisError>
    where
        K: redis::ToRedisArgs,
        V: serde::Serialize + vld::schema::VldParse + ?Sized,
    {
        let payload = Self::encode_json_string(value)?;
        let len: usize = redis::cmd("LPUSH").arg(key).arg(payload).query(&mut self.inner)?;
        Ok(len)
    }

    pub fn rpush<K, V>(&mut self, key: K, value: &V) -> Result<usize, VldRedisError>
    where
        K: redis::ToRedisArgs,
        V: serde::Serialize + vld::schema::VldParse + ?Sized,
    {
        let payload = Self::encode_json_string(value)?;
        let len: usize = redis::cmd("RPUSH").arg(key).arg(payload).query(&mut self.inner)?;
        Ok(len)
    }

    pub fn lpop<K, T>(&mut self, key: K) -> Result<Option<T>, VldRedisError>
    where
        K: redis::ToRedisArgs,
        T: vld::schema::VldParse,
    {
        let raw: Option<Vec<u8>> = redis::cmd("LPOP").arg(key).query(&mut self.inner)?;
        raw.map(|bytes| Self::decode_json_bytes(&bytes)).transpose()
    }

    pub fn rpop<K, T>(&mut self, key: K) -> Result<Option<T>, VldRedisError>
    where
        K: redis::ToRedisArgs,
        T: vld::schema::VldParse,
    {
        let raw: Option<Vec<u8>> = redis::cmd("RPOP").arg(key).query(&mut self.inner)?;
        raw.map(|bytes| Self::decode_json_bytes(&bytes)).transpose()
    }

    pub fn sadd<K, V>(&mut self, key: K, value: &V) -> Result<usize, VldRedisError>
    where
        K: redis::ToRedisArgs,
        V: serde::Serialize + vld::schema::VldParse + ?Sized,
    {
        let payload = Self::encode_json_string(value)?;
        let added: usize = redis::cmd("SADD").arg(key).arg(payload).query(&mut self.inner)?;
        Ok(added)
    }

    pub fn smembers<K, T>(&mut self, key: K) -> Result<Vec<T>, VldRedisError>
    where
        K: redis::ToRedisArgs,
        T: vld::schema::VldParse,
    {
        let raw: Vec<Vec<u8>> = redis::cmd("SMEMBERS").arg(key).query(&mut self.inner)?;
        raw.into_iter()
            .map(|bytes| Self::decode_json_bytes(&bytes))
            .collect()
    }

    pub fn zadd<K, V>(&mut self, key: K, score: f64, value: &V) -> Result<usize, VldRedisError>
    where
        K: redis::ToRedisArgs,
        V: serde::Serialize + vld::schema::VldParse + ?Sized,
    {
        let payload = Self::encode_json_string(value)?;
        let added: usize = redis::cmd("ZADD")
            .arg(key)
            .arg(score)
            .arg(payload)
            .query(&mut self.inner)?;
        Ok(added)
    }

    pub fn zrange<K, T>(&mut self, key: K, start: isize, stop: isize) -> Result<Vec<T>, VldRedisError>
    where
        K: redis::ToRedisArgs,
        T: vld::schema::VldParse,
    {
        let raw: Vec<Vec<u8>> = redis::cmd("ZRANGE")
            .arg(key)
            .arg(start)
            .arg(stop)
            .query(&mut self.inner)?;
        raw.into_iter()
            .map(|bytes| Self::decode_json_bytes(&bytes))
            .collect()
    }

    pub fn publish<Cn, V>(&mut self, channel: Cn, value: &V) -> Result<i64, VldRedisError>
    where
        Cn: redis::ToRedisArgs,
        V: serde::Serialize + vld::schema::VldParse + ?Sized,
    {
        let payload = Self::encode_json_bytes(value)?;
        let delivered: i64 = redis::cmd("PUBLISH")
            .arg(channel)
            .arg(payload)
            .query(&mut self.inner)?;
        Ok(delivered)
    }
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

/// Rebind Redis connection into `vld`-aware connection with native-like calls.
#[macro_export]
macro_rules! impl_to_redis {
    ($conn:ident) => {
        let mut $conn = $crate::RedisConn::new($conn);
    };
}

pub mod prelude {
    pub use crate::VldRedisError;
    pub use crate::{impl_to_redis, RedisConn};
    pub use vld::prelude::*;
}
