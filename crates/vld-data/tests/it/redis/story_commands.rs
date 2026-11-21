//! Stories: every validating command against the fake server, valid values
//! written as JSON and read back.

use super::common::*;
use super::vld_redis::RedisConn;

/// Given a profile cache keyed by user id,
/// when users are written with `set` / `mset` and read with `get` / `mget`,
/// then each value is stored as the validated JSON, reads give the users
/// back, and missing keys read as `None`.
#[test]
fn strings_round_trip() {
    let alice = user("Alice", "alice@example.com");
    let bob = user("Bob", "bob@example.com");
    let mut conn = RedisConn::new(FakeRedis::new());

    conn.set("user:1", &alice).unwrap();
    assert!(matches!(&conn.data[b"user:1".as_slice()], Entry::Str(b) if *b == stored(&alice)));
    assert_eq!(
        conn.get::<_, UserSchema>("user:1").unwrap(),
        Some(alice.clone())
    );
    assert_eq!(conn.get::<_, UserSchema>("user:404").unwrap(), None);

    conn.mset([("user:2", &bob), ("user:3", &alice)]).unwrap();
    let many: Vec<Option<UserSchema>> = conn.mget(["user:2", "user:404", "user:3"]).unwrap();
    assert_eq!(many, [Some(bob), None, Some(alice)]);
    assert_eq!(conn.commands(), ["SET", "GET", "GET", "MSET", "MGET"]);
}

/// Given a hash of users per team,
/// when members are written with `hset` and read with `hget`,
/// then each field holds the validated JSON and a missing field is `None`.
#[test]
fn hashes_round_trip() {
    let alice = user("Alice", "alice@example.com");
    let mut conn = RedisConn::new(FakeRedis::new());
    conn.hset("team:1", "lead", &alice).unwrap();
    assert_eq!(
        conn.hget::<_, _, UserSchema>("team:1", "lead").unwrap(),
        Some(alice)
    );
    assert_eq!(
        conn.hget::<_, _, UserSchema>("team:1", "dev").unwrap(),
        None
    );
    assert_eq!(
        conn.hget::<_, _, UserSchema>("team:2", "lead").unwrap(),
        None
    );
}

/// Given a job queue,
/// when users are pushed on both ends and popped from both ends,
/// then the pushes return the list length, pops give the users in deque
/// order, and an empty list pops `None`.
#[test]
fn lists_behave_as_deques() {
    let [a, b, c] = [
        user("A", "a@example.com"),
        user("B", "b@example.com"),
        user("C", "c@example.com"),
    ];
    let mut conn = RedisConn::new(FakeRedis::new());
    assert_eq!(conn.rpush("q", &a).unwrap(), 1);
    assert_eq!(conn.rpush("q", &b).unwrap(), 2);
    assert_eq!(conn.lpush("q", &c).unwrap(), 3);
    assert_eq!(conn.list("q"), [stored(&c), stored(&a), stored(&b)]);
    assert_eq!(conn.lpop::<_, UserSchema>("q").unwrap(), Some(c));
    assert_eq!(conn.rpop::<_, UserSchema>("q").unwrap(), Some(b));
    assert_eq!(conn.rpop::<_, UserSchema>("q").unwrap(), Some(a));
    assert_eq!(conn.lpop::<_, UserSchema>("q").unwrap(), None);
    assert_eq!(conn.rpop::<_, UserSchema>("q").unwrap(), None);
}

/// Given a set and a sorted set of users,
/// when members are added (one twice) and read back,
/// then `sadd` / `zadd` return how many were new, `smembers` gives each
/// member once, and `zrange` gives them by score, honouring the range.
#[test]
fn sets_and_sorted_sets() {
    let [a, b, c] = [
        user("A", "a@example.com"),
        user("B", "b@example.com"),
        user("C", "c@example.com"),
    ];
    let mut conn = RedisConn::new(FakeRedis::new());
    assert_eq!(conn.sadd("s", &a).unwrap(), 1);
    assert_eq!(conn.sadd("s", &b).unwrap(), 1);
    assert_eq!(conn.sadd("s", &a).unwrap(), 0);
    let mut members: Vec<UserSchema> = conn.smembers("s").unwrap();
    members.sort_by(|x, y| x.name.cmp(&y.name));
    assert_eq!(members, [a.clone(), b.clone()]);
    assert_eq!(conn.smembers::<_, UserSchema>("none").unwrap(), []);

    assert_eq!(conn.zadd("z", 3.0, &a).unwrap(), 1);
    assert_eq!(conn.zadd("z", 1.5, &b).unwrap(), 1);
    assert_eq!(conn.zadd("z", 2.0, &c).unwrap(), 1);
    assert_eq!(conn.zadd("z", 0.5, &a).unwrap(), 0);
    let all: Vec<UserSchema> = conn.zrange("z", 0, -1).unwrap();
    assert_eq!(all, [a, b.clone(), c.clone()]);
    let tail: Vec<UserSchema> = conn.zrange("z", 1, 2).unwrap();
    assert_eq!(tail, [b, c]);
}

/// Given a channel with two subscribers,
/// when a user event is published,
/// then the subscribers receive the validated JSON and `publish` returns the
/// subscriber count.
#[test]
fn publish_sends_validated_json() {
    let alice = user("Alice", "alice@example.com");
    let mut fake = FakeRedis::new();
    fake.subscribers = 2;
    let mut conn = RedisConn::new(fake);
    assert_eq!(conn.publish("events", &alice).unwrap(), 2);
    let fake = conn.into_inner();
    assert_eq!(fake.published, [(b"events".to_vec(), stored(&alice))]);
}

/// Given the wrapper, which shadows `get` / `set` / … of `redis::Commands`,
/// when native commands are called through deref and through the trait on
/// the inner connection,
/// then commands without a validating counterpart (`del`) work directly and
/// the native `get` returns the raw stored text.
#[test]
fn native_commands_stay_reachable() {
    use super::vld_redis::redis::Commands;
    let mut conn = RedisConn::new(FakeRedis::new());
    let _: () = Commands::set(&mut *conn, "raw", "raw text").unwrap();
    let raw: String = Commands::get(&mut *conn, "raw").unwrap();
    assert_eq!(raw, "raw text");
    let removed: usize = conn.del("raw").unwrap();
    assert_eq!(removed, 1);
    assert_eq!(conn.get::<_, UserSchema>("raw").unwrap(), None);
}
