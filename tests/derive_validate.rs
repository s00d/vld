//! Tests for `#[derive(Validate)]` from the `vld-derive` crate.
#![cfg(feature = "derive")]

use vld::prelude::*;
use vld::Validate;

// ---------------------------------------------------------------------------
// Basic derive
// ---------------------------------------------------------------------------

#[derive(Debug, Validate)]
struct SimpleUser {
    #[vld(vld::string().min(2).max(50))]
    name: String,
    #[vld(vld::string().email())]
    email: String,
    #[vld(vld::number().int().min(0).optional())]
    age: Option<i64>,
}

#[test]
fn derive_basic_valid() {
    let user = SimpleUser::parse_value(&serde_json::json!({
        "name": "Alice",
        "email": "alice@example.com",
        "age": 30
    }))
    .unwrap();
    assert_eq!(user.name, "Alice");
    assert_eq!(user.email, "alice@example.com");
    assert_eq!(user.age, Some(30));
}

#[test]
fn derive_basic_optional_missing() {
    let user = SimpleUser::parse_value(&serde_json::json!({
        "name": "Bob",
        "email": "bob@example.com"
    }))
    .unwrap();
    assert_eq!(user.age, None);
}

#[test]
fn derive_basic_invalid() {
    let err = SimpleUser::parse_value(&serde_json::json!({
        "name": "X",
        "email": "not-email",
        "age": -5
    }))
    .unwrap_err();
    // Should have errors for name (too short), email (invalid), age (too small)
    assert!(err.issues.len() >= 2);
}

#[test]
fn derive_vld_parse_trait() {
    // VldParse trait should be implemented
    let user = SimpleUser::vld_parse_value(&serde_json::json!({
        "name": "Alice",
        "email": "alice@example.com"
    }))
    .unwrap();
    assert_eq!(user.name, "Alice");
}

#[test]
fn derive_vld_parse_method() {
    let user = SimpleUser::vld_parse(r#"{"name": "Alice", "email": "alice@example.com"}"#).unwrap();
    assert_eq!(user.name, "Alice");
}

// ---------------------------------------------------------------------------
// validate_fields / parse_lenient
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Validate)]
struct Config {
    #[vld(vld::string().min(1))]
    host: String,
    #[vld(vld::number().int().min(1).max(65535))]
    port: i64,
}

#[test]
fn derive_validate_fields() {
    let results = Config::validate_fields_value(&serde_json::json!({
        "host": "",
        "port": 0
    }))
    .unwrap();
    assert_eq!(results.len(), 2);
    assert!(results[0].is_err()); // host empty
    assert!(results[1].is_err()); // port < 1
}

#[test]
fn derive_parse_lenient() {
    let result = Config::parse_lenient_value(&serde_json::json!({
        "host": "",
        "port": 0
    }))
    .unwrap();
    assert!(result.has_errors());
    // Invalid fields fall back to Default
    assert_eq!(result.value.host, ""); // String::default
    assert_eq!(result.value.port, 0); // i64::default
}

// ---------------------------------------------------------------------------
// Serde rename support
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Serialize, Validate)]
struct CamelUser {
    #[serde(rename = "firstName")]
    #[vld(vld::string().min(2))]
    first_name: String,
    #[vld(vld::string().email())]
    email: String,
}

#[test]
fn derive_serde_rename() {
    let user = CamelUser::parse_value(&serde_json::json!({
        "firstName": "John",
        "email": "john@example.com"
    }))
    .unwrap();
    assert_eq!(user.first_name, "John");
}

#[test]
fn derive_serde_rename_missing_original_key() {
    // Using "first_name" instead of "firstName" should fail
    let err = CamelUser::parse_value(&serde_json::json!({
        "first_name": "John",
        "email": "john@example.com"
    }))
    .unwrap_err();
    assert!(!err.issues.is_empty());
}

// ---------------------------------------------------------------------------
// Non-object input
// ---------------------------------------------------------------------------

#[test]
fn derive_non_object_input() {
    let err = SimpleUser::parse_value(&serde_json::json!("not an object")).unwrap_err();
    assert!(err.issues[0].message.contains("object"));
}

#[test]
fn derive_array_input() {
    let err = SimpleUser::parse_value(&serde_json::json!([1, 2, 3])).unwrap_err();
    assert!(err.issues[0].message.contains("object"));
}
