//! Stories: invalid values never reach Redis, invalid stored data is
//! reported, and Redis failures surface as `VldRedisError::Redis`.

use crate::common::*;
use std::error::Error as _;
use vld_redis::redis::Commands;
use vld_redis::{RedisConn, VldRedisError};

/// Given an invalid user (empty name, bad email),
/// when it is written with every validating write command (and inside an
/// `mset` with a valid one),
/// then each call fails with both issues and no command reaches the server.
#[test]
fn invalid_values_are_never_sent() {
    let good = user("Alice", "alice@example.com");
    let bad = user("", "nope");
    let both = pairs(&[(".name", "too_small"), (".email", "invalid_string")]);
    let mut conn = RedisConn::new(FakeRedis::new());

    assert_eq!(issues(conn.set("k", &bad)), both);
    assert_eq!(issues(conn.mset([("a", &good), ("b", &bad)])), both);
    assert_eq!(issues(conn.hset("h", "f", &bad)), both);
    assert_eq!(issues(conn.lpush("l", &bad)), both);
    assert_eq!(issues(conn.rpush("l", &bad)), both);
    assert_eq!(issues(conn.sadd("s", &bad)), both);
    assert_eq!(issues(conn.zadd("z", 1.0, &bad)), both);
    assert_eq!(issues(conn.publish("c", &bad)), both);
    assert!(conn.log.is_empty(), "{:?}", conn.commands());
    assert!(conn.data.is_empty());
}

/// Given data written by another client: a user that breaks the schema and
/// text that is not JSON,
/// when it is read with every validating read command,
/// then the bad user reports every bad path, non-JSON is a
/// `Deserialization` error, and a collection fails on its first bad element.
#[test]
fn invalid_stored_data_is_reported() {
    let good = br#"{"name":"A","email":"a@b.co"}"#.as_slice();
    let bad = br#"{"name":"","email":"x"}"#.as_slice();
    let both = pairs(&[(".name", "too_small"), (".email", "invalid_string")]);
    let mut conn = RedisConn::new(FakeRedis::new());
    let _: () = Commands::set(&mut *conn, "bad", bad).unwrap();
    let _: () = Commands::set(&mut *conn, "text", "plain text").unwrap();
    let _: () = Commands::hset(&mut *conn, "h", "f", bad).unwrap();
    for v in [good, bad] {
        let _: () = Commands::rpush(&mut *conn, "l", v).unwrap();
        let _: () = Commands::sadd(&mut *conn, "s", v).unwrap();
        let _: () = Commands::zadd(&mut *conn, "z", v, 1).unwrap();
    }

    assert_eq!(issues(conn.get::<_, UserSchema>("bad")), both);
    assert!(matches!(
        conn.get::<_, UserSchema>("text"),
        Err(VldRedisError::Deserialization(_))
    ));
    assert_eq!(issues(conn.mget::<_, UserSchema, _>(["bad"])), both);
    assert_eq!(issues(conn.hget::<_, _, UserSchema>("h", "f")), both);
    assert_eq!(issues(conn.smembers::<_, UserSchema>("s")), both);
    assert_eq!(issues(conn.zrange::<_, UserSchema>("z", 0, -1)), both);
    assert_eq!(issues(conn.rpop::<_, UserSchema>("l")), both);
    assert!(conn.lpop::<_, UserSchema>("l").unwrap().is_some());
}

/// A value that refuses to serialize (but has a schema).
#[derive(Debug)]
struct Unserializable;
impl serde::Serialize for Unserializable {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("nope"))
    }
}
impl vld::schema::VldParse for Unserializable {
    fn vld_parse_value(_: &serde_json::Value) -> Result<Self, vld::error::VldError> {
        Ok(Self)
    }
}

/// Given a server that fails the next command, a key of the wrong type and
/// a value that cannot be serialized,
/// when they are written / read,
/// then server failures are `Redis` errors with the `RedisError` as source,
/// and the unserializable value is a `Serialization` error that sends
/// nothing; each error's text names its kind.
#[test]
fn failures_are_typed_errors() {
    let alice = user("Alice", "alice@example.com");
    let mut conn = RedisConn::new(FakeRedis::new());

    conn.fail_next = Some("connection reset".into());
    let err = conn.set("k", &alice).unwrap_err();
    assert!(matches!(err, VldRedisError::Redis(_)), "{err:?}");
    assert!(err.source().is_some());
    assert!(err.to_string().starts_with("Redis error: "), "{err}");
    assert!(err.to_string().contains("connection reset"), "{err}");

    conn.rpush("list", &alice).unwrap();
    assert!(matches!(
        conn.get::<_, UserSchema>("list"),
        Err(VldRedisError::Redis(_))
    ));

    let sent = conn.log.len();
    let err = conn.set("k", &Unserializable).unwrap_err();
    assert!(
        matches!(&err, VldRedisError::Serialization(m) if m == "nope"),
        "{err:?}"
    );
    assert_eq!(err.to_string(), "Serialization error: nope");
    assert!(err.source().is_none());
    assert_eq!(conn.log.len(), sent);

    let _: () = Commands::set(&mut *conn, "text", "{").unwrap();
    let err = conn.get::<_, UserSchema>("text").unwrap_err();
    assert!(
        err.to_string().starts_with("Deserialization error: "),
        "{err}"
    );
    assert!(err.source().is_none());

    let err: VldRedisError =
        vld::error::VldError::single(vld::error::IssueCode::MissingField, "required").into();
    assert_eq!(issues::<()>(Err(err)), pairs(&[("", "missing_field")]));
    let err = VldRedisError::Validation(vld::error::VldError::single(
        vld::error::IssueCode::MissingField,
        "required",
    ));
    assert!(err.to_string().starts_with("Validation error: "), "{err}");
    assert!(err.source().is_some());
}
