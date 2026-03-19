//! Basic example of using vld-sqlx for validation before insert.
//!
//! Run: cargo run -p vld-sqlx --example sqlx_basic

use sqlx::{Row, SqlitePool};
use vld_sqlx::prelude::*;

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

vld::schema! {
    #[derive(Debug)]
    pub struct AgeField {
        pub value: i64 => vld::number().int().min(0).max(150),
    }
}

#[derive(Debug, serde::Serialize)]
struct NewUser {
    name: String,
    email: String,
    age: i64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== vld-sqlx example ===\n");

    let pool = SqlitePool::connect(":memory:").await?;

    sqlx::query(
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            age INTEGER NOT NULL
        )",
    )
    .execute(&pool)
    .await?;

    // --- 1. validate_insert ---
    println!("1. validate_insert:");
    let good = NewUser {
        name: "Alice".into(),
        email: "alice@example.com".into(),
        age: 30,
    };
    match validate_insert::<NewUserSchema, _>(&good) {
        Ok(()) => {
            println!("   [OK] Validation passed for {:?}", good);
            sqlx::query("INSERT INTO users (name, email, age) VALUES (?, ?, ?)")
                .bind(&good.name)
                .bind(&good.email)
                .bind(good.age)
                .execute(&pool)
                .await?;
            println!("   [OK] Inserted into DB");
        }
        Err(e) => println!("   [ERR] {}", e),
    }

    let bad = NewUser {
        name: "".into(),
        email: "not-email".into(),
        age: -5,
    };
    match validate_insert::<NewUserSchema, _>(&bad) {
        Ok(()) => println!("   Unexpected success"),
        Err(e) => println!("   [OK] Correctly rejected: {}", e),
    }

    // --- 2. Validated wrapper ---
    println!("\n2. Validated wrapper:");
    let user = NewUser {
        name: "Bob".into(),
        email: "bob@example.com".into(),
        age: 25,
    };
    match Validated::<NewUserSchema, _>::new(user) {
        Ok(validated) => {
            println!("   [OK] Validated: {:?}", validated);
            sqlx::query("INSERT INTO users (name, email, age) VALUES (?, ?, ?)")
                .bind(&validated.inner().name)
                .bind(&validated.inner().email)
                .bind(validated.inner().age)
                .execute(&pool)
                .await?;
            println!("   [OK] Inserted into DB");
        }
        Err(e) => println!("   [ERR] {}", e),
    }

    // --- 3. VldText typed column ---
    println!("\n3. VldText<EmailField>:");
    match VldText::<EmailField>::new("carol@example.com") {
        Ok(email) => {
            println!("   [OK] Valid email: {}", email);
            sqlx::query("INSERT INTO users (name, email, age) VALUES ('Carol', ?, 28)")
                .bind(&email)
                .execute(&pool)
                .await?;
            println!("   [OK] Inserted via VldText bind");
        }
        Err(e) => println!("   [ERR] {}", e),
    }
    match VldText::<EmailField>::new("bad") {
        Ok(_) => println!("   Unexpected success"),
        Err(e) => println!("   [OK] Correctly rejected: {}", e),
    }

    // --- 4. VldInt typed column ---
    println!("\n4. VldInt<AgeField>:");
    match VldInt::<AgeField>::new(42) {
        Ok(age) => println!("   [OK] Valid age: {}", age),
        Err(e) => println!("   [ERR] {}", e),
    }
    match VldInt::<AgeField>::new(-1) {
        Ok(_) => println!("   Unexpected success"),
        Err(e) => println!("   [OK] Correctly rejected: {}", e),
    }

    // --- 5. validate_row on loaded data ---
    println!("\n5. validate_row on loaded data:");
    let rows = sqlx::query("SELECT name, email, age FROM users")
        .fetch_all(&pool)
        .await?;

    for row in &rows {
        let user = NewUser {
            name: row.get("name"),
            email: row.get("email"),
            age: row.get("age"),
        };
        match validate_row::<NewUserSchema, _>(&user) {
            Ok(()) => println!("   [OK] {} — valid", user.name),
            Err(e) => println!("   [ERR] {} — {}", user.name, e),
        }
    }

    // --- 6. Decode VldText from DB ---
    println!("\n6. Decode VldText from DB:");
    let row = sqlx::query("SELECT email FROM users LIMIT 1")
        .fetch_one(&pool)
        .await?;
    let decoded: VldText<EmailField> = row.get("email");
    println!("   [OK] Decoded email: {}", decoded);

    println!(
        "\n=== Done. {} users in DB ===",
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
            .fetch_one(&pool)
            .await?
    );

    Ok(())
}
