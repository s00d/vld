//! Real Redis example for `RedisConn`.
//!
//! Run:
//! cargo run -p vld-redis --example redis_basic

use vld_redis::prelude::*;
use vld_redis::redis;

vld::schema! {
    #[derive(Debug, serde::Serialize)]
    pub struct UserSchema {
        pub name: String => vld::string().min(1).max(100),
        pub email: String => vld::string().email(),
        pub age: i64 => vld::number().int().min(0).max(150),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== vld-redis example ===\n");

    let good = UserSchema {
        name: "Alice".into(),
        email: "alice@example.com".into(),
        age: 30,
    };
    let bad = UserSchema {
        name: "".into(),
        email: "not-an-email".into(),
        age: -5,
    };

    println!("1) real Redis connection (RedisConn API):");
    let client = redis::Client::open("redis://127.0.0.1/")?;
    let mut conn = RedisConn::new(client.get_connection()?);

    conn.set("vld:user:1", &good)?;
    let loaded: Option<UserSchema> = conn.get("vld:user:1")?;
    println!(
        "   [OK] GET vld:user:1 => {:?}",
        loaded.as_ref().map(|u| &u.email)
    );

    conn.hset("vld:users", "good", &good)?;
    let from_hash: Option<UserSchema> = conn.hget("vld:users", "good")?;
    println!(
        "   [OK] HGET vld:users/good => {:?}",
        from_hash.as_ref().map(|u| &u.name)
    );

    let subscribers = conn.publish("vld:users:events", &good)?;
    println!("   [OK] PUBLISH delivered to {subscribers} subscriber(s)");

    println!("\n2) invalid payload is rejected before Redis write:");
    match conn.set("vld:user:bad", &bad) {
        Ok(_) => println!("   Unexpected success"),
        Err(e) => println!("   [OK] Validation failed: {e}"),
    }

    Ok(())
}
