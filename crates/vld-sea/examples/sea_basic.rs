//! Example: validate ActiveModel before insert/update with vld-sea.
//!
//! This example does NOT connect to a database — it demonstrates the
//! validation flow only.

use sea_orm::Set;
use vld_sea::prelude::*;

// ---------------------------------------------------------------------------
// 1. Define the vld validation schema
// ---------------------------------------------------------------------------

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct UserInput {
        pub name: String  => vld::string().min(1).max(100),
        pub email: String => vld::string().email(),
    }
}

// ---------------------------------------------------------------------------
// 2. Define the SeaORM entity
// ---------------------------------------------------------------------------

mod user {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "users")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub name: String,
        pub email: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    // Option A: manual before_save
    impl ActiveModelBehavior for ActiveModel {}

    // Option B: automatic validation via macro (uncomment to use):
    // vld_sea::impl_vld_before_save!(ActiveModel, super::UserInput);
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    println!("=== vld-sea example ===\n");

    // --- Valid ActiveModel ---
    let am = user::ActiveModel {
        id: Set(1),
        name: Set("Alice".to_owned()),
        email: Set("alice@example.com".to_owned()),
    };

    println!("ActiveModel JSON: {}", vld_sea::active_model_to_json(&am));

    match validate_active::<UserInput, _>(&am) {
        Ok(parsed) => println!("Valid! Parsed: {:?}", parsed),
        Err(e) => println!("ERROR: {}", e),
    }

    // --- Invalid ActiveModel ---
    let bad = user::ActiveModel {
        id: Set(2),
        name: Set("".to_owned()),
        email: Set("not-an-email".to_owned()),
    };

    println!(
        "\nInvalid ActiveModel JSON: {}",
        vld_sea::active_model_to_json(&bad)
    );

    match validate_active::<UserInput, _>(&bad) {
        Ok(_) => println!("Unexpected: should have failed"),
        Err(e) => println!("Expected error: {}", e),
    }

    // --- Validate a DTO (Serialize-based) ---
    #[derive(Debug, serde::Serialize)]
    struct NewUser {
        name: String,
        email: String,
    }

    let dto = NewUser {
        name: "Bob".into(),
        email: "bob@example.com".into(),
    };

    match validate_model::<UserInput, _>(&dto) {
        Ok(parsed) => println!("\nDTO valid! Parsed: {:?}", parsed),
        Err(e) => println!("\nDTO error: {}", e),
    }

    // --- before_save helper ---
    println!("\n--- before_save helper ---");
    let am3 = user::ActiveModel {
        id: Set(3),
        name: Set("Charlie".to_owned()),
        email: Set("charlie@test.com".to_owned()),
    };

    match vld_sea::before_save::<UserInput, _>(&am3) {
        Ok(()) => println!("before_save: OK, ready to insert"),
        Err(e) => println!("before_save: BLOCKED — {}", e),
    }

    // --- Validated wrapper ---
    println!("\n--- Validated wrapper ---");
    let input = NewUser {
        name: "Dave".into(),
        email: "dave@example.com".into(),
    };
    match Validated::<UserInput, _>::new(input) {
        Ok(v) => println!("Validated wrapper: {:?}", v),
        Err(e) => println!("Validated wrapper error: {}", e),
    }
}
