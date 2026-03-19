use vld_surrealdb::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct PersonSchema {
        pub name: String  => vld::string().min(1).max(100),
        pub email: String => vld::string().email(),
        pub age: i64      => vld::number().int().min(0).max(150),
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct Person {
    name: String,
    email: String,
    age: i64,
}

fn main() {
    println!("=== vld-surrealdb example ===\n");

    // 1. Validate before create/insert
    let valid_person = Person {
        name: "Alice".into(),
        email: "alice@example.com".into(),
        age: 30,
    };

    match validate_content::<PersonSchema, _>(&valid_person) {
        Ok(()) => println!("[OK] Person is valid, safe to db.create(\"person\").content(...)"),
        Err(e) => println!("[ERR] {}", e),
    }

    // 2. Validate invalid data
    let bad_person = Person {
        name: "".into(),
        email: "not-an-email".into(),
        age: -5,
    };

    match validate_content::<PersonSchema, _>(&bad_person) {
        Ok(()) => println!("[OK] Person is valid"),
        Err(e) => {
            println!("[ERR] Validation failed:");
            let response = VldSurrealResponse::from_error(&e);
            for field in &response.fields {
                println!("  - {}: {}", field.field, field.message);
            }
        }
    }

    // 3. Validate raw JSON (e.g. from SurrealQL query results)
    let json = serde_json::json!({"name": "Bob", "email": "bob@example.com", "age": 25});
    match validate_json::<PersonSchema>(&json) {
        Ok(()) => println!("\n[OK] JSON document is valid"),
        Err(e) => println!("[ERR] {}", e),
    }

    // 4. Validated wrapper
    let v = Validated::<PersonSchema, _>::new(valid_person).unwrap();
    println!("\n[OK] Validated person: {} (age {})", v.name, v.age);

    // 5. Typed field wrappers
    vld::schema! {
        #[derive(Debug)]
        pub struct EmailField {
            pub value: String => vld::string().email(),
        }
    }

    let email = VldText::<EmailField>::new("hello@world.com").unwrap();
    println!("[OK] Validated email: {}", email);

    // 6. validate_fields! macro
    let name = "Charlie";
    let age = 25i64;
    let result = validate_fields! {
        name => vld::string().min(1).max(100),
        age => vld::number().int().min(0).max(150),
    };
    println!("\n[OK] Fields valid: {}", result.is_ok());

    println!("\n=== Example complete ===");
}
