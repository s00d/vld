
vld::schema! {
    #[derive(Debug, serde::Serialize)]
    pub struct UserSchema {
        pub name: String => vld::string().min(1),
        pub email: String => vld::string().email(),
    }
}

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let client = redis::Client::open("redis://127.0.0.1/")?;
let mut conn = RedisConn::new(client.get_connection()?);

conn.set("user:1", &UserSchema { name: "Alice".into(), email: "alice@example.com".into() })?;
let user: Option<UserSchema> = conn.get("user:1")?;

let bad = UserSchema { name: "".into(), email: "nope".into() };
assert!(matches!(conn.set("user:2", &bad), Err(VldRedisError::Validation(_))));
# Ok(()) }
