use serde::Serialize;
use vld_dioxus::{check_all_fields, check_field, check_field_all, validate, FieldError, VldServerError};

// ===================== VldServerError ========================================

#[test]
fn vld_server_error_validation() {
    let err = VldServerError::validation(vec![
        FieldError { field: "name".into(), message: "too short".into() },
        FieldError { field: "email".into(), message: "invalid email".into() },
    ]);
    assert_eq!(err.fields.len(), 2);
    assert_eq!(err.field_error("name"), Some("too short"));
    assert_eq!(err.field_error("email"), Some("invalid email"));
    assert_eq!(err.field_error("age"), None);
    assert!(err.has_field_error("name"));
    assert!(!err.has_field_error("age"));
}

#[test]
fn vld_server_error_field_errors_all() {
    let err = VldServerError::validation(vec![
        FieldError { field: "name".into(), message: "too short".into() },
        FieldError { field: "name".into(), message: "must start with uppercase".into() },
    ]);
    let msgs = err.field_errors("name");
    assert_eq!(msgs.len(), 2);
}

#[test]
fn vld_server_error_error_fields() {
    let err = VldServerError::validation(vec![
        FieldError { field: "name".into(), message: "x".into() },
        FieldError { field: "email".into(), message: "y".into() },
    ]);
    let fields = err.error_fields();
    assert!(fields.contains(&"name"));
    assert!(fields.contains(&"email"));
}

#[test]
fn vld_server_error_display_is_json() {
    let err = VldServerError::validation(vec![
        FieldError { field: "name".into(), message: "too short".into() },
    ]);
    let display = err.to_string();
    let parsed: VldServerError = serde_json::from_str(&display).unwrap();
    assert_eq!(parsed, err);
}

#[test]
fn vld_server_error_from_json() {
    let err = VldServerError::validation(vec![
        FieldError { field: "age".into(), message: "too small".into() },
    ]);
    let json = serde_json::to_string(&err).unwrap();
    let parsed = VldServerError::from_json(&json).unwrap();
    assert_eq!(parsed, err);
    assert!(VldServerError::from_json("not json").is_none());
}

#[test]
fn vld_server_error_internal() {
    let err = VldServerError::internal("something broke");
    assert_eq!(err.message, "something broke");
    assert!(err.fields.is_empty());
}

// ===================== From<VldError> ========================================

#[test]
fn from_vld_error_conversion() {
    vld::schema! {
        #[derive(Debug)]
        struct TestSchema {
            name: String => vld::string().min(5),
        }
    }

    let json = serde_json::json!({ "name": "AB" });
    let vld_err = TestSchema::parse_value(&json).unwrap_err();
    let server_err: VldServerError = vld_err.into();

    assert!(!server_err.fields.is_empty());
    assert!(server_err.fields.iter().any(|f| f.field.contains("name")));
}

// ===================== validate / validate_value =============================

vld::schema! {
    struct UserSchema {
        name: String => vld::string().min(2).max(50),
        email: String => vld::string().email(),
    }
}

#[derive(Serialize)]
struct UserArgs {
    name: String,
    email: String,
}

#[test]
fn validate_valid() {
    let args = UserArgs { name: "Alice".into(), email: "alice@example.com".into() };
    assert!(validate::<UserSchema, _>(&args).is_ok());
}

#[test]
fn validate_invalid_name() {
    let args = UserArgs { name: "A".into(), email: "alice@example.com".into() };
    let err = validate::<UserSchema, _>(&args).unwrap_err();
    assert!(err.has_field_error(".name"));
    assert!(!err.has_field_error(".email"));
}

#[test]
fn validate_invalid_email() {
    let args = UserArgs { name: "Alice".into(), email: "bad".into() };
    let err = validate::<UserSchema, _>(&args).unwrap_err();
    assert!(err.has_field_error(".email"));
}

#[test]
fn validate_multiple_errors() {
    let args = UserArgs { name: "A".into(), email: "bad".into() };
    let err = validate::<UserSchema, _>(&args).unwrap_err();
    assert!(err.has_field_error(".name"));
    assert!(err.has_field_error(".email"));
}

#[test]
fn validate_value_ok() {
    let json = serde_json::json!({ "name": "Alice", "email": "a@b.com" });
    assert!(vld_dioxus::validate_value::<UserSchema>(&json).is_ok());
}

#[test]
fn validate_value_err() {
    let json = serde_json::json!({ "name": "A", "email": "a@b.com" });
    let err = vld_dioxus::validate_value::<UserSchema>(&json).unwrap_err();
    assert!(err.has_field_error(".name"));
}

// ===================== check_field ===========================================

#[test]
fn check_field_valid() {
    assert!(check_field(&"Alice".to_string(), &vld::string().min(2)).is_none());
}

#[test]
fn check_field_invalid() {
    assert!(check_field(&"A".to_string(), &vld::string().min(2)).is_some());
}

#[test]
fn check_field_number() {
    assert!(check_field(&25, &vld::number().int().min(0).max(150)).is_none());
    assert!(check_field(&-5, &vld::number().int().min(0)).is_some());
}

#[test]
fn check_field_all_multiple() {
    let errors = check_field_all(&"".to_string(), &vld::string().min(2));
    assert!(!errors.is_empty());
}

// ===================== check_all_fields ======================================

#[test]
fn check_all_fields_valid() {
    let data = UserArgs { name: "Alice".into(), email: "a@b.com".into() };
    assert!(check_all_fields::<UserSchema, _>(&data).is_empty());
}

#[test]
fn check_all_fields_invalid() {
    let data = UserArgs { name: "A".into(), email: "bad".into() };
    let errors = check_all_fields::<UserSchema, _>(&data);
    assert!(errors.len() >= 2);
    assert!(errors.iter().any(|e| e.field.contains("name")));
    assert!(errors.iter().any(|e| e.field.contains("email")));
}

// ===================== validate_args! macro ==================================

#[test]
fn validate_args_macro_valid() {
    let name = "Alice".to_string();
    let email = "alice@example.com".to_string();
    let age: i64 = 25;

    let result = vld_dioxus::validate_args! {
        name  => vld::string().min(2).max(50),
        email => vld::string().email(),
        age   => vld::number().int().min(0).max(150),
    };
    assert!(result.is_ok());
}

#[test]
fn validate_args_macro_invalid() {
    let name = "A".to_string();
    let email = "bad".to_string();
    let age: i64 = -1;

    let err = vld_dioxus::validate_args! {
        name  => vld::string().min(2).max(50),
        email => vld::string().email(),
        age   => vld::number().int().min(0).max(150),
    }
    .unwrap_err();

    assert!(err.has_field_error("name"));
    assert!(err.has_field_error("email"));
    assert!(err.has_field_error("age"));
}

#[test]
fn validate_args_macro_partial_error() {
    let name = "Alice".to_string();
    let email = "bad".to_string();

    let err = vld_dioxus::validate_args! {
        name  => vld::string().min(2),
        email => vld::string().email(),
    }
    .unwrap_err();

    assert!(!err.has_field_error("name"));
    assert!(err.has_field_error("email"));
}

// ===================== Roundtrip (server→client error transport) ==============

#[test]
fn error_roundtrip_via_json_string() {
    let name = "A".to_string();
    let email = "bad".to_string();

    let err = vld_dioxus::validate_args! {
        name  => vld::string().min(2),
        email => vld::string().email(),
    }
    .unwrap_err();

    let error_string = err.to_string();
    let recovered = VldServerError::from_json(&error_string).unwrap();
    assert!(recovered.has_field_error("name"));
    assert!(recovered.has_field_error("email"));
}

// ===================== Shared schema pattern =================================

fn name_schema() -> vld::primitives::ZString {
    vld::string().min(2).max(50)
}

fn email_schema() -> vld::primitives::ZString {
    vld::string().email()
}

#[test]
fn shared_schema_server_side() {
    let name = "Alice".to_string();
    let email = "alice@test.com".to_string();

    let result = vld_dioxus::validate_args! {
        name  => name_schema(),
        email => email_schema(),
    };
    assert!(result.is_ok());
}

#[test]
fn shared_schema_client_side() {
    assert!(check_field(&"Alice".to_string(), &name_schema()).is_none());
    assert!(check_field(&"A".to_string(), &name_schema()).is_some());
    assert!(check_field(&"a@b.com".to_string(), &email_schema()).is_none());
    assert!(check_field(&"bad".to_string(), &email_schema()).is_some());
}
