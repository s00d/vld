//! # vld-leptos basic example
//!
//! Demonstrates server-side and client-side validation patterns.
//! This example runs as a regular binary (no Leptos runtime needed).
//!
//! ```sh
//! cargo run -p vld-leptos --example leptos_basic
//! ```

use serde::Serialize;

// ── Shared validation schemas (compile for both server and WASM) ─────────────

fn name_schema() -> vld::primitives::ZString {
    vld::string().min(2).max(50)
}

fn email_schema() -> vld::primitives::ZString {
    vld::string().email()
}

fn age_schema() -> vld::primitives::ZInt {
    vld::number().int().min(0).max(150)
}

// ── Schema struct (alternative to individual schemas) ────────────────────────

vld::schema! {
    struct CreateUserSchema {
        name: String => vld::string().min(2).max(50),
        email: String => vld::string().email(),
        age: i64 => vld::number().int().min(0).max(150),
    }
}

#[derive(Serialize)]
struct CreateUserArgs {
    name: String,
    email: String,
    age: i64,
}

fn main() {
    println!("=== vld-leptos validation patterns ===\n");

    // ── Pattern 1: validate_args! macro (for server functions) ───────────────

    println!("--- Pattern 1: validate_args! macro ---");
    {
        let name = "Alice".to_string();
        let email = "alice@example.com".to_string();
        let age: i64 = 25;

        match vld_leptos::validate_args! {
            name  => name_schema(),
            email => email_schema(),
            age   => age_schema(),
        } {
            Ok(()) => println!("[OK] Valid: name={}, email={}, age={}", name, email, age),
            Err(e) => println!("[ERR] {}", e),
        }
    }
    {
        let name = "A".to_string();
        let email = "bad".to_string();
        let age: i64 = -5;

        match vld_leptos::validate_args! {
            name  => name_schema(),
            email => email_schema(),
            age   => age_schema(),
        } {
            Ok(()) => println!("[OK] Unexpected success"),
            Err(e) => {
                println!("[ERR] Server error: {}", e.message);
                for f in &e.fields {
                    println!("  - {}: {}", f.field, f.message);
                }
            }
        }
    }

    // ── Pattern 2: Schema-based validation ───────────────────────────────────

    println!("\n--- Pattern 2: Schema-based validation ---");
    {
        let args = CreateUserArgs {
            name: "Bob".into(),
            email: "bob@test.com".into(),
            age: 30,
        };
        match vld_leptos::validate::<CreateUserSchema, _>(&args) {
            Ok(()) => println!("[OK] Schema validation passed"),
            Err(e) => println!("[ERR] {}", e),
        }
    }

    // ── Pattern 3: Client-side field validation ──────────────────────────────

    println!("\n--- Pattern 3: check_field (client-side) ---");
    {
        let values = ["Alice", "A", ""];
        for v in &values {
            let err = vld_leptos::check_field(&v.to_string(), &name_schema());
            match err {
                None => println!("[OK]  name=\"{}\"", v),
                Some(msg) => println!("[ERR] name=\"{}\" → {}", v, msg),
            }
        }
    }

    // ── Pattern 4: Whole-form validation ─────────────────────────────────────

    println!("\n--- Pattern 4: check_all_fields (whole form) ---");
    {
        let args = CreateUserArgs {
            name: "X".into(),
            email: "bad".into(),
            age: 200,
        };
        let errors = vld_leptos::check_all_fields::<CreateUserSchema, _>(&args);
        if errors.is_empty() {
            println!("[OK]  All fields valid");
        } else {
            println!("[ERR] {} field error(s):", errors.len());
            for e in &errors {
                println!("  - {}: {}", e.field, e.message);
            }
        }
    }

    // ── Pattern 5: Error roundtrip (server→client) ───────────────────────────

    println!("\n--- Pattern 5: Error roundtrip ---");
    {
        let name = "".to_string();
        let email = "x".to_string();

        let err = vld_leptos::validate_args! {
            name  => name_schema(),
            email => email_schema(),
        }
        .unwrap_err();

        let json_str = err.to_string();
        println!("  Server → JSON: {}", json_str);

        let recovered = vld_leptos::VldServerError::from_json(&json_str).unwrap();
        println!(
            "  Client ← parsed: {} field error(s)",
            recovered.fields.len()
        );
        if let Some(msg) = recovered.field_error("name") {
            println!("  name error: {}", msg);
        }
        if let Some(msg) = recovered.field_error("email") {
            println!("  email error: {}", msg);
        }
    }

    println!("\n=== Done ===");
}
