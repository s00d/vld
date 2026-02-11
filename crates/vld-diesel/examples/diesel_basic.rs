//! Basic example of using vld-diesel for validation before insert.
//!
//! Run: cargo run -p vld-diesel --example diesel_basic

use diesel::prelude::*;
use vld_diesel::prelude::*;

// ---------------------------------------------------------------------------
// vld schemas
// ---------------------------------------------------------------------------

vld::schema! {
    #[derive(Debug)]
    pub struct NewUserSchema {
        pub name: String  => vld::string().min(1).max(100),
        pub email: String => vld::string().email(),
        pub age: i64      => vld::number().int().min(0).max(150),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct EmailField {
        pub value: String => vld::string().email(),
    }
}

// ---------------------------------------------------------------------------
// Diesel table + models
// ---------------------------------------------------------------------------

diesel::table! {
    users (id) {
        id -> Integer,
        name -> Text,
        email -> Text,
        age -> BigInt,
    }
}

#[derive(Debug, Insertable, serde::Serialize)]
#[diesel(table_name = users)]
struct NewUser {
    name: String,
    email: String,
    age: i64,
}

#[derive(Debug, Queryable, Selectable, serde::Serialize)]
#[diesel(table_name = users)]
struct User {
    id: i32,
    name: String,
    email: String,
    age: i64,
}

fn main() {
    println!("=== vld-diesel example ===\n");

    // Set up in-memory SQLite
    let mut conn = SqliteConnection::establish(":memory:").expect("Failed to connect to SQLite");

    diesel::sql_query(
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            age BIGINT NOT NULL
        )",
    )
    .execute(&mut conn)
    .unwrap();

    // --- 1. validate_insert (standalone function) ---
    println!("1. validate_insert:");
    let good = NewUser {
        name: "Alice".into(),
        email: "alice@example.com".into(),
        age: 30,
    };
    match validate_insert::<NewUserSchema, _>(&good) {
        Ok(()) => {
            println!("   ✔ Validation passed for {:?}", good);
            diesel::insert_into(users::table)
                .values(&good)
                .execute(&mut conn)
                .unwrap();
            println!("   ✔ Inserted into DB");
        }
        Err(e) => println!("   ✖ {}", e),
    }

    let bad = NewUser {
        name: "".into(),
        email: "not-email".into(),
        age: -5,
    };
    match validate_insert::<NewUserSchema, _>(&bad) {
        Ok(()) => println!("   Unexpected success"),
        Err(e) => println!("   ✔ Correctly rejected: {}", e),
    }

    // --- 2. Validated<S, T> wrapper ---
    println!("\n2. Validated wrapper:");
    let user = NewUser {
        name: "Bob".into(),
        email: "bob@example.com".into(),
        age: 25,
    };
    match Validated::<NewUserSchema, _>::new(user) {
        Ok(validated) => {
            println!("   ✔ Validated: {:?}", validated);
            diesel::insert_into(users::table)
                .values(validated.inner())
                .execute(&mut conn)
                .unwrap();
            println!("   ✔ Inserted into DB");
        }
        Err(e) => println!("   ✖ {}", e),
    }

    // --- 3. VldText typed column ---
    println!("\n3. VldText<EmailField>:");
    match VldText::<EmailField>::new("carol@example.com") {
        Ok(email) => println!("   ✔ Valid email: {}", email),
        Err(e) => println!("   ✖ {}", e),
    }
    match VldText::<EmailField>::new("bad") {
        Ok(_) => println!("   Unexpected success"),
        Err(e) => println!("   ✔ Correctly rejected: {}", e),
    }

    // --- 4. validate_row (check existing data) ---
    println!("\n4. validate_row on loaded data:");
    let loaded: Vec<User> = users::table
        .select(User::as_select())
        .load(&mut conn)
        .unwrap();

    for user in &loaded {
        match validate_row::<NewUserSchema, _>(user) {
            Ok(()) => println!("   ✔ {} — valid", user.name),
            Err(e) => println!("   ✖ {} — {}", user.name, e),
        }
    }

    println!("\n=== Done. {} users in DB ===", loaded.len());
}
