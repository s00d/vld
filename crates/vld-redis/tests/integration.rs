use vld_redis::redis::{self, Value};
use vld_redis::{RedisConn, VldRedisError};

vld::schema! {
    #[derive(Debug, serde::Serialize)]
    pub struct UserSchema {
        pub name: String => vld::string().min(1),
        pub email: String => vld::string().email(),
    }
}

/// Fake connection: records sent commands and answers them with `replies` in
/// order (the last one repeats).
struct Mock {
    replies: Vec<Value>,
    sent: Vec<Vec<u8>>,
}

impl Mock {
    fn new(reply: Value) -> Self {
        Self::script(vec![reply])
    }
    fn script(replies: Vec<Value>) -> Self {
        Self {
            replies,
            sent: Vec::new(),
        }
    }
    fn next(&mut self) -> Value {
        if self.replies.len() > 1 {
            self.replies.remove(0)
        } else {
            self.replies[0].clone()
        }
    }
    /// The last command as text (RESP framing included).
    fn last(&self) -> String {
        String::from_utf8_lossy(self.sent.last().unwrap()).into_owned()
    }
}

impl redis::ConnectionLike for Mock {
    fn req_packed_command(&mut self, cmd: &[u8]) -> redis::RedisResult<Value> {
        self.sent.push(cmd.to_vec());
        Ok(self.next())
    }
    fn req_packed_commands(
        &mut self,
        cmd: &[u8],
        _offset: usize,
        _count: usize,
    ) -> redis::RedisResult<Vec<Value>> {
        self.sent.push(cmd.to_vec());
        Ok(vec![self.next()])
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
// Every command: what is sent and how the reply is decoded
// ---------------------------------------------------------------------------

const ALICE: &str = r#"{"name":"Alice","email":"alice@example.com"}"#;
/// How `alice()` is written (validated JSON value, keys sorted).
const SENT: &str = r#"{"email":"alice@example.com","name":"Alice"}"#;

fn alice() -> UserSchema {
    UserSchema {
        name: "Alice".into(),
        email: "alice@example.com".into(),
    }
}

#[test]
fn every_command_sends_json_and_decodes_replies() {
    let user = alice();
    let mut conn = RedisConn::new(Mock::script(vec![
        Value::Okay,                                              // SET
        bulk(ALICE),                                              // GET
        Value::Nil,                                               // GET missing
        Value::Okay,                                              // MSET
        Value::Array(vec![bulk(ALICE), bulk(ALICE), Value::Nil]), // MGET
        Value::Int(1),                                            // HSET
        bulk(ALICE),                                              // HGET
        Value::Int(1),                                            // LPUSH
        Value::Int(2),                                            // RPUSH
        bulk(ALICE),                                              // LPOP
        Value::Nil,                                               // RPOP empty
        Value::Int(1),                                            // SADD
        Value::Array(vec![bulk(ALICE)]),                          // SMEMBERS
        Value::Int(1),                                            // ZADD
        Value::Array(vec![bulk(ALICE)]),                          // ZRANGE
        Value::Int(0),                                            // PUBLISH
    ]));

    conn.set("k", &user).unwrap();
    assert!(conn.last().contains("SET") && conn.last().contains(SENT));
    let got: Option<UserSchema> = conn.get("k").unwrap();
    assert_eq!(got.unwrap().email, "alice@example.com");
    let missing: Option<UserSchema> = conn.get("missing").unwrap();
    assert!(missing.is_none());

    conn.mset([("m1", &user), ("m2", &user)]).unwrap();
    assert!(conn.last().contains("MSET"));
    let many: Vec<Option<UserSchema>> = conn.mget(["m1", "m2", "m3"]).unwrap();
    assert_eq!(many.iter().filter(|u| u.is_some()).count(), 2);

    conn.hset("h", "a", &user).unwrap();
    assert!(conn.last().contains("HSET") && conn.last().contains(SENT));
    let h: Option<UserSchema> = conn.hget("h", "a").unwrap();
    assert_eq!(h.unwrap().name, "Alice");

    assert_eq!(conn.lpush("l", &user).unwrap(), 1);
    assert_eq!(conn.rpush("l", &user).unwrap(), 2);
    let l: Option<UserSchema> = conn.lpop("l").unwrap();
    assert!(l.is_some());
    let empty: Option<UserSchema> = conn.rpop("l").unwrap();
    assert!(empty.is_none());

    assert_eq!(conn.sadd("s", &user).unwrap(), 1);
    let members: Vec<UserSchema> = conn.smembers("s").unwrap();
    assert_eq!(members.len(), 1);

    assert_eq!(conn.zadd("z", 1.0, &user).unwrap(), 1);
    let zs: Vec<UserSchema> = conn.zrange("z", 0, -1).unwrap();
    assert_eq!(zs[0].name, "Alice");

    let subscribers: i64 = conn.publish("chan", &user).unwrap();
    assert_eq!(subscribers, 0);
    assert!(conn.last().contains("PUBLISH") && conn.last().contains(SENT));
}

#[test]
fn invalid_stored_value_reports_every_path() {
    let mut conn = RedisConn::new(Mock::new(bulk(r#"{"name":"","email":"x"}"#)));
    match conn.get::<_, UserSchema>("bad").unwrap_err() {
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
    // A collection fails on its first invalid element.
    let mut conn = RedisConn::new(Mock::new(Value::Array(vec![
        bulk(ALICE),
        bulk(r#"{"name":"","email":"x"}"#),
    ])));
    assert!(matches!(
        conn.smembers::<_, UserSchema>("s"),
        Err(VldRedisError::Validation(_))
    ));
}
