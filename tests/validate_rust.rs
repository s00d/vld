#![cfg(feature = "serialize")]

use vld::prelude::*;

// ---------------------------------------------------------------------------
// VldSchema::validate() / is_valid() — validate existing Rust values
// ---------------------------------------------------------------------------

#[test]
fn validate_vec_ok() {
    let schema = vld::array(vld::number().int().positive()).min_len(1);
    assert!(schema.validate(&vec![1, 2, 3]).is_ok());
}

#[test]
fn validate_vec_err() {
    let schema = vld::array(vld::number().int().positive()).min_len(1);
    let err = schema.validate(&vec![-1, 0]).unwrap_err();
    assert!(!err.issues.is_empty());
}

#[test]
fn is_valid_string() {
    let schema = vld::string().email();
    assert!(schema.is_valid(&"user@example.com"));
    assert!(!schema.is_valid(&"not-an-email"));
}

#[test]
fn is_valid_number() {
    let schema = vld::number().min(0.0).max(100.0);
    assert!(schema.is_valid(&50));
    assert!(!schema.is_valid(&200));
}

#[test]
fn validate_hashmap_as_record() {
    let schema = vld::record(vld::number().int().positive());
    let mut map = std::collections::HashMap::new();
    map.insert("a", 1);
    map.insert("b", 2);
    assert!(schema.validate(&map).is_ok());
}

// ---------------------------------------------------------------------------
// schema! validate() / is_valid() — validate struct instances
// ---------------------------------------------------------------------------

vld::schema! {
    #[derive(Debug, serde::Serialize)]
    struct TestUser {
        name: String => vld::string().min(2).max(50),
        email: String => vld::string().email(),
        age: i64 => vld::number().int().min(18),
    }
}

#[test]
fn schema_validate_valid_instance() {
    let user = TestUser::parse(r#"{"name":"Alice","email":"a@b.com","age":25}"#).unwrap();
    assert!(TestUser::validate(&user).is_ok());
    assert!(TestUser::is_valid(&user));
}

#[test]
fn schema_validate_invalid_instance() {
    let bad = TestUser {
        name: "A".to_string(),
        email: "bad".to_string(),
        age: 10,
    };
    assert!(!TestUser::is_valid(&bad));
    let err = TestUser::validate(&bad).unwrap_err();
    assert!(err.issues.len() >= 3); // name, email, age
}

#[test]
fn schema_validate_json_value() {
    let json = serde_json::json!({
        "name": "Bob",
        "email": "bob@test.com",
        "age": 30
    });
    assert!(TestUser::is_valid(&json));

    let bad_json = serde_json::json!({
        "name": "X",
        "email": "bad",
        "age": 5
    });
    assert!(!TestUser::is_valid(&bad_json));
}

// ---------------------------------------------------------------------------
// Struct without Serialize — validate/is_valid not available but parse works
// ---------------------------------------------------------------------------

vld::schema! {
    #[derive(Debug)]
    struct NoSerialize {
        val: String => vld::string().min(1),
    }
}

#[test]
fn no_serialize_parse_still_works() {
    let result = NoSerialize::parse(r#"{"val": "hello"}"#);
    assert!(result.is_ok());
}
