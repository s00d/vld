use vld_redis::redis::{self, Value};
use vld_redis::{RedisConn, VldRedisError};

vld::schema! {
    #[derive(Debug, serde::Serialize)]
    pub struct UserSchema {
        pub name: String => vld::string().min(1),
        pub email: String => vld::string().email(),
    }
}

/// Fake connection: records sent commands and answers every request with `reply`.
struct Mock {
    reply: Value,
    sent: Vec<Vec<u8>>,
}

impl Mock {
    fn new(reply: Value) -> Self {
        Self {
            reply,
            sent: Vec::new(),
        }
    }
}

impl redis::ConnectionLike for Mock {
    fn req_packed_command(&mut self, cmd: &[u8]) -> redis::RedisResult<Value> {
        self.sent.push(cmd.to_vec());
        Ok(self.reply.clone())
    }
    fn req_packed_commands(
        &mut self,
        cmd: &[u8],
        _offset: usize,
        _count: usize,
    ) -> redis::RedisResult<Vec<Value>> {
        self.sent.push(cmd.to_vec());
        Ok(vec![self.reply.clone()])
    }
    fn get_db(&self) -> i64 {
        0
    }
    fn check_connection(&mut self) -> bool {
        true
    }
    fn is_open(&self) -> bool {
        true
    }
}

fn bulk(s: &str) -> Value {
    Value::BulkString(s.as_bytes().to_vec())
}

#[test]
fn set_rejects_invalid_value_without_sending() {
    let mut conn = RedisConn::new(Mock::new(Value::Okay));
    let bad = UserSchema {
        name: String::new(),
        email: "nope".into(),
    };
    assert!(matches!(
        conn.set("k", &bad),
        Err(VldRedisError::Validation(_))
    ));
    assert!(conn.sent.is_empty());
}

#[test]
fn get_validates_and_decodes() {
    let mut conn = RedisConn::new(Mock::new(bulk(r#"{"name":"A","email":"a@b.co"}"#)));
    let u: Option<UserSchema> = conn.get("k").unwrap();
    assert_eq!(u.unwrap().name, "A");

    let mut conn = RedisConn::new(Mock::new(bulk(r#"{"name":"","email":"x"}"#)));
    assert!(matches!(
        conn.get::<_, UserSchema>("k"),
        Err(VldRedisError::Validation(_))
    ));
}

#[test]
fn native_command_reachable_through_trait_on_inner() {
    // `conn.get` is the validating one; the native one is reached via the trait.
    let mut conn = RedisConn::new(Mock::new(bulk("raw text")));
    let raw: String = redis::Commands::get(&mut *conn, "k").unwrap();
    assert_eq!(raw, "raw text");
}

#[cfg(feature = "aio")]
mod aio {
    use super::*;
    use vld_redis::AsyncRedisConn;

    struct AsyncMock(Value);

    impl redis::aio::ConnectionLike for AsyncMock {
        fn req_packed_command<'a>(
            &'a mut self,
            _cmd: &'a redis::Cmd,
        ) -> redis::RedisFuture<'a, Value> {
            let v = self.0.clone();
            Box::pin(async move { Ok(v) })
        }
        fn req_packed_commands<'a>(
            &'a mut self,
            _cmd: &'a redis::Pipeline,
            _offset: usize,
            _count: usize,
        ) -> redis::RedisFuture<'a, Vec<Value>> {
            let v = self.0.clone();
            Box::pin(async move { Ok(vec![v]) })
        }
        fn get_db(&self) -> i64 {
            0
        }
    }

    #[tokio::test]
    async fn async_get_and_set_validate() {
        let mut conn = AsyncRedisConn::new(AsyncMock(bulk(r#"{"name":"A","email":"a@b.co"}"#)));
        let u: Option<UserSchema> = conn.get("k").await.unwrap();
        assert_eq!(u.unwrap().email, "a@b.co");

        let bad = UserSchema {
            name: String::new(),
            email: "x".into(),
        };
        assert!(matches!(
            conn.set("k", &bad).await,
            Err(VldRedisError::Validation(_))
        ));
        let mut conn = AsyncRedisConn::new(AsyncMock(Value::Nil));
        let none: Option<UserSchema> = conn.lpop("k").await.unwrap();
        assert!(none.is_none());
    }
}

// ---------------------------------------------------------------------------
// Live server tests: run when REDIS_URL is set (CI starts a redis service).
// Without it they fail when `CI` is set (unless `VLD_LIVE_OPTIONAL` is set,
// for CI jobs without the service) and otherwise print a note and pass.
// ---------------------------------------------------------------------------

fn redis_url() -> Option<String> {
    match std::env::var("REDIS_URL") {
        Ok(url) if !url.is_empty() => Some(url),
        _ => {
            let in_ci = std::env::var_os("CI").is_some();
            let optional = std::env::var_os("VLD_LIVE_OPTIONAL").is_some();
            assert!(
                !in_ci || optional,
                "CI is set but REDIS_URL is missing: the live redis tests would silently skip \
                 (set VLD_LIVE_OPTIONAL=1 in CI jobs without a redis service)"
            );
            eprintln!("note: REDIS_URL not set, skipping live redis test");
            None
        }
    }
}

/// Unique key prefix so parallel runs never collide.
fn prefix() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("vld-test:{}:{nanos}", std::process::id())
}

fn alice() -> UserSchema {
    UserSchema {
        name: "Alice".into(),
        email: "alice@example.com".into(),
    }
}

#[test]
fn live_round_trips_every_command() {
    let Some(url) = redis_url() else { return };
    let client = redis::Client::open(url).unwrap();
    let mut conn = RedisConn::new(client.get_connection().unwrap());
    let p = prefix();
    let k = |s: &str| format!("{p}:{s}");
    let user = alice();

    conn.set(k("str"), &user).unwrap();
    let got: Option<UserSchema> = conn.get(k("str")).unwrap();
    assert_eq!(got.unwrap().email, "alice@example.com");
    let missing: Option<UserSchema> = conn.get(k("missing")).unwrap();
    assert!(missing.is_none());

    conn.mset([(k("m1"), &user), (k("m2"), &user)]).unwrap();
    let many: Vec<Option<UserSchema>> = conn.mget([k("m1"), k("m2"), k("m3")]).unwrap();
    assert_eq!(many.iter().filter(|u| u.is_some()).count(), 2);
    assert!(many[2].is_none());

    conn.hset(k("h"), "a", &user).unwrap();
    let h: Option<UserSchema> = conn.hget(k("h"), "a").unwrap();
    assert_eq!(h.unwrap().name, "Alice");

    assert_eq!(conn.lpush(k("l"), &user).unwrap(), 1);
    assert_eq!(conn.rpush(k("l"), &user).unwrap(), 2);
    let l: Option<UserSchema> = conn.lpop(k("l")).unwrap();
    let r: Option<UserSchema> = conn.rpop(k("l")).unwrap();
    assert!(l.is_some() && r.is_some());
    let empty: Option<UserSchema> = conn.lpop(k("l")).unwrap();
    assert!(empty.is_none());

    assert_eq!(conn.sadd(k("s"), &user).unwrap(), 1);
    let members: Vec<UserSchema> = conn.smembers(k("s")).unwrap();
    assert_eq!(members.len(), 1);

    assert_eq!(conn.zadd(k("z"), 1.0, &user).unwrap(), 1);
    let zs: Vec<UserSchema> = conn.zrange(k("z"), 0, -1).unwrap();
    assert_eq!(zs[0].name, "Alice");

    let subscribers: i64 = conn.publish(k("chan"), &user).unwrap();
    assert_eq!(subscribers, 0);

    // A value written by someone else that violates the schema is rejected on read.
    redis::Commands::set::<_, _, ()>(&mut *conn, k("bad"), r#"{"name":"","email":"x"}"#).unwrap();
    let err = conn.get::<_, UserSchema>(k("bad")).unwrap_err();
    match err {
        VldRedisError::Validation(e) => {
            let paths: Vec<String> = e
                .issues
                .iter()
                .map(|i| i.path.iter().map(|s| s.to_string()).collect())
                .collect();
            assert_eq!(paths, [".name", ".email"]);
        }
        other => panic!("expected a validation error, got {other:?}"),
    }

    // Invalid values are never written.
    let bad = UserSchema {
        name: String::new(),
        email: "nope".into(),
    };
    assert!(matches!(
        conn.set(k("never"), &bad),
        Err(VldRedisError::Validation(_))
    ));
    let exists: bool = redis::Commands::exists(&mut *conn, k("never")).unwrap();
    assert!(!exists);

    let keys: Vec<String> = redis::Commands::keys(&mut *conn, format!("{p}:*")).unwrap();
    let _: () = redis::Commands::del(&mut *conn, keys).unwrap();
}

#[cfg(feature = "aio")]
#[tokio::test]
async fn live_async_round_trip() {
    use vld_redis::AsyncRedisConn;
    let Some(url) = redis_url() else { return };
    let client = redis::Client::open(url).unwrap();
    let mut conn = AsyncRedisConn::new(client.get_multiplexed_async_connection().await.unwrap());
    let key = format!("{}:async", prefix());

    conn.set(&key, &alice()).await.unwrap();
    let got: Option<UserSchema> = conn.get(&key).await.unwrap();
    assert_eq!(got.unwrap().name, "Alice");
    let _: () = redis::AsyncCommands::del(&mut *conn, &key).await.unwrap();
}
