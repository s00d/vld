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
        mut conn: vld_redis::RedisConn<redis::Connection>,
        user: &UserSchema,
    ) -> Result<(), vld_redis::VldRedisError> {
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

    let _ = compile_check
        as fn(
            vld_redis::RedisConn<redis::Connection>,
            &UserSchema,
        ) -> Result<(), vld_redis::VldRedisError>;
}
