//! Stories: the async wrapper (`AsyncRedisConn`, feature `aio`) against the
//! same fake server.
#![cfg(feature = "redis-aio")]

use super::common::*;
use super::vld_redis::prelude::*;

/// Given an async connection,
/// when users go through every validating command and an invalid user is
/// written,
/// then the results match the sync wrapper's: values round-trip as validated
/// JSON, missing / empty reads are `None`, and the invalid write fails with
/// its issues before anything is sent.
#[tokio::test]
async fn async_commands_round_trip_and_reject() {
    let [a, b] = [user("A", "a@example.com"), user("B", "b@example.com")];
    let bad = user("", "x");
    let mut fake = FakeRedis::new();
    fake.subscribers = 1;
    let mut conn = AsyncRedisConn::new(fake);

    conn.set("k", &a).await.unwrap();
    assert_eq!(
        conn.get::<_, UserSchema>("k").await.unwrap(),
        Some(a.clone())
    );
    conn.mset([("m1", &a), ("m2", &b)]).await.unwrap();
    let many: Vec<Option<UserSchema>> = conn.mget(["m1", "x", "m2"]).await.unwrap();
    assert_eq!(many, [Some(a.clone()), None, Some(b.clone())]);
    conn.hset("h", "f", &b).await.unwrap();
    assert_eq!(
        conn.hget::<_, _, UserSchema>("h", "f").await.unwrap(),
        Some(b.clone())
    );
    assert_eq!(conn.rpush("l", &a).await.unwrap(), 1);
    assert_eq!(conn.lpush("l", &b).await.unwrap(), 2);
    assert_eq!(
        conn.rpop::<_, UserSchema>("l").await.unwrap(),
        Some(a.clone())
    );
    assert_eq!(
        conn.lpop::<_, UserSchema>("l").await.unwrap(),
        Some(b.clone())
    );
    assert_eq!(conn.lpop::<_, UserSchema>("l").await.unwrap(), None);
    assert_eq!(conn.sadd("s", &a).await.unwrap(), 1);
    assert_eq!(
        conn.smembers::<_, UserSchema>("s").await.unwrap(),
        std::slice::from_ref(&a)
    );
    assert_eq!(conn.zadd("z", 2.0, &a).await.unwrap(), 1);
    assert_eq!(conn.zadd("z", 1.0, &b).await.unwrap(), 1);
    assert_eq!(
        conn.zrange::<_, UserSchema>("z", 0, -1).await.unwrap(),
        [b, a.clone()]
    );
    assert_eq!(conn.publish("c", &a).await.unwrap(), 1);

    let sent = conn.log.len();
    assert_eq!(
        issues(conn.set("k", &bad).await),
        pairs(&[(".name", "too_small"), (".email", "invalid_string")])
    );
    assert_eq!(conn.log.len(), sent);
    let fake = conn.into_inner();
    assert_eq!(fake.published, [(b"c".to_vec(), stored(&a))]);
}
