use vld_redis::redis::{self, Value};
use vld_redis::{RedisConn, VldRedisError};

vld::schema! {
    #[derive(Debug, serde::Serialize)]
    pub struct UserSchema {
        pub name: String => vld::string().min(1),
        pub email: String => vld::string().email(),
    }
}

#[test]
fn impl_to_redis_api_compiles() {
    fn compile_check(
        mut conn: RedisConn<redis::Connection>,
        user: &UserSchema,
    ) -> Result<(), VldRedisError> {
        conn.set("user:1", user)?;
        let _loaded: Option<UserSchema> = conn.get("user:1")?;
        conn.mset([("user:2", user), ("user:3", user)])?;
        let _loaded_many: Vec<Option<UserSchema>> = conn.mget(["user:2", "user:3"])?;
        conn.hset("users:hash", "good", user)?;
        let _from_hash: Option<UserSchema> = conn.hget("users:hash", "good")?;
        let _list_len = conn.lpush("users:list", user)?;
        let _list_len2 = conn.rpush("users:list", user)?;
        let _popped_l: Option<UserSchema> = conn.lpop("users:list")?;
        let _popped_r: Option<UserSchema> = conn.rpop("users:list")?;
        let _set_added = conn.sadd("users:set", user)?;
        let _set_members: Vec<UserSchema> = conn.smembers("users:set")?;
        let _zset_added = conn.zadd("users:zset", 1.0, user)?;
        let _zset_members: Vec<UserSchema> = conn.zrange("users:zset", 0, -1)?;
        let _subscribers: i64 = conn.publish("users", user)?;
        Ok(())
    }

    let _ =
        compile_check as fn(RedisConn<redis::Connection>, &UserSchema) -> Result<(), VldRedisError>;
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
