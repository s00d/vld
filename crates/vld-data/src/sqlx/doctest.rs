
vld::schema! {
    #[derive(Debug)]
    pub struct NewUserSchema {
        pub name: String  => vld::string().min(1).max(100),
        pub email: String => vld::string().email(),
    }
}

// Column schemas validate `{"value": ...}`.
vld::schema! {
    #[derive(Debug)]
    pub struct EmailField {
        pub value: String => vld::string().email(),
    }
}

#[derive(serde::Serialize)]
struct NewUser { name: String, email: String }

let user = NewUser { name: "Alice".into(), email: "alice@example.com".into() };
validate_insert::<NewUserSchema, _>(&user).unwrap();
// sqlx::query("INSERT ...").bind(&user.name).bind(&user.email).execute(&pool).await?;

let v = Validated::<NewUserSchema, _>::new(user).unwrap();
assert_eq!(v.inner().name, "Alice");

let email = VldText::<EmailField>::new("carol@example.com").unwrap(); // bind(&email)
assert!(VldText::<EmailField>::new("bad").is_err());
# let _ = email;
