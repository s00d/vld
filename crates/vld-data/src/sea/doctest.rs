
vld::schema! {
    #[derive(Debug)]
    pub struct UserInput {
        pub name: String  => vld::string().min(1).max(100),
        pub email: String => vld::string().email(),
    }
}

#[derive(serde::Serialize)]
struct NewUser { name: String, email: String }

let dto = NewUser { name: "Alice".into(), email: "alice@example.com".into() };
let parsed = validate_model::<UserInput, _>(&dto).unwrap();
assert_eq!(parsed.name, "Alice");
