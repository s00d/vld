use serde_json::json;
use vld::prelude::*;
use vld_surrealdb::*;

// ========================= Schemas ===========================================

vld::schema! {
    #[derive(Debug)]
    pub struct PersonSchema {
        pub name: String  => vld::string().min(1).max(100),
        pub email: String => vld::string().email(),
        pub age: i64      => vld::number().int().min(0).max(150),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct NameSchema {
        pub name: String => vld::string().min(1).max(50),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct OptionalSchema {
        pub name: String => vld::string().min(1),
        pub bio: Option<String> => vld::string().optional(),
    }
}

// Field schemas for typed wrappers
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

vld::schema! {
    #[derive(Debug)]
    pub struct PriceField {
        pub value: f64 => vld::number().min(0.0),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct ActiveField {
        pub value: bool => vld::boolean(),
    }
}

// ========================= Test data =========================================

#[derive(serde::Serialize, serde::Deserialize)]
struct Person {
    name: String,
    email: String,
    age: i64,
}

#[derive(serde::Serialize)]
struct NameOnly {
    name: String,
}

// ========================= validate_content ==================================

#[test]
fn validate_content_valid() {
    let person = Person {
        name: "Alice".into(),
        email: "alice@example.com".into(),
        age: 30,
    };
    assert!(validate_content::<PersonSchema, _>(&person).is_ok());
}

#[test]
fn validate_content_invalid_name() {
    let person = Person {
        name: "".into(),
        email: "alice@example.com".into(),
        age: 30,
    };
    assert!(validate_content::<PersonSchema, _>(&person).is_err());
}

#[test]
fn validate_content_invalid_email() {
    let person = Person {
        name: "Alice".into(),
        email: "not-an-email".into(),
        age: 30,
    };
    assert!(validate_content::<PersonSchema, _>(&person).is_err());
}

#[test]
fn validate_content_invalid_age() {
    let person = Person {
        name: "Alice".into(),
        email: "alice@example.com".into(),
        age: -1,
    };
    assert!(validate_content::<PersonSchema, _>(&person).is_err());
}

// ========================= validate_json =====================================

#[test]
fn validate_json_valid() {
    let json = json!({"name": "Bob", "email": "bob@example.com", "age": 25});
    assert!(validate_json::<PersonSchema>(&json).is_ok());
}

#[test]
fn validate_json_invalid() {
    let json = json!({"name": "", "email": "bad", "age": 200});
    assert!(validate_json::<PersonSchema>(&json).is_err());
}

#[test]
fn validate_json_missing_fields() {
    let json = json!({"name": "Bob"});
    assert!(validate_json::<PersonSchema>(&json).is_err());
}

// ========================= validate_record(s) ================================

#[test]
fn validate_record_valid() {
    let p = Person {
        name: "Charlie".into(),
        email: "charlie@example.com".into(),
        age: 40,
    };
    assert!(validate_record::<PersonSchema, _>(&p).is_ok());
}

#[test]
fn validate_records_all_valid() {
    let rows = vec![
        NameOnly {
            name: "Alice".into(),
        },
        NameOnly { name: "Bob".into() },
    ];
    assert!(validate_records::<NameSchema, _>(&rows).is_ok());
}

#[test]
fn validate_records_one_invalid() {
    let rows = vec![
        NameOnly {
            name: "Alice".into(),
        },
        NameOnly { name: "".into() },
        NameOnly {
            name: "Charlie".into(),
        },
    ];
    let err = validate_records::<NameSchema, _>(&rows).unwrap_err();
    assert_eq!(err.0, 1); // index of bad row
}

// ========================= validate_value ====================================

#[test]
fn validate_value_valid_string() {
    let schema = vld::string().min(1).max(100);
    let val = json!("Hello");
    assert!(validate_value(&schema, &val).is_ok());
}

#[test]
fn validate_value_invalid_string() {
    let schema = vld::string().min(5);
    let val = json!("Hi");
    assert!(validate_value(&schema, &val).is_err());
}

#[test]
fn validate_value_valid_number() {
    let schema = vld::number().int().min(0).max(100);
    let val = json!(42);
    assert!(validate_value(&schema, &val).is_ok());
}

// ========================= Validated<S, T> ===================================

#[test]
fn validated_wrapper_valid() {
    let p = Person {
        name: "Alice".into(),
        email: "alice@example.com".into(),
        age: 30,
    };
    let v = Validated::<PersonSchema, _>::new(p).unwrap();
    assert_eq!(v.inner().name, "Alice");
    assert_eq!(v.name, "Alice"); // Deref
}

#[test]
fn validated_wrapper_invalid() {
    let p = Person {
        name: "".into(),
        email: "bad".into(),
        age: -1,
    };
    assert!(Validated::<PersonSchema, _>::new(p).is_err());
}

#[test]
fn validated_into_inner() {
    let p = Person {
        name: "Bob".into(),
        email: "bob@example.com".into(),
        age: 25,
    };
    let v = Validated::<PersonSchema, _>::new(p).unwrap();
    let inner = v.into_inner();
    assert_eq!(inner.name, "Bob");
}

#[test]
fn validated_serializes() {
    let p = Person {
        name: "Alice".into(),
        email: "alice@example.com".into(),
        age: 30,
    };
    let v = Validated::<PersonSchema, _>::new(p).unwrap();
    let json = serde_json::to_value(&v).unwrap();
    assert_eq!(json["name"], "Alice");
    assert_eq!(json["age"], 30);
}

// ========================= VldText<S> ========================================

#[test]
fn vld_text_valid() {
    let email = VldText::<EmailField>::new("user@example.com").unwrap();
    assert_eq!(email.as_str(), "user@example.com");
    assert_eq!(&*email, "user@example.com"); // Deref
}

#[test]
fn vld_text_invalid() {
    assert!(VldText::<EmailField>::new("not-email").is_err());
}

#[test]
fn vld_text_serializes() {
    let email = VldText::<EmailField>::new("test@test.com").unwrap();
    let json = serde_json::to_value(&email).unwrap();
    assert_eq!(json, "test@test.com");
}

#[test]
fn vld_text_deserializes_valid() {
    let json = json!("valid@email.com");
    let email: VldText<EmailField> = serde_json::from_value(json).unwrap();
    assert_eq!(email.as_str(), "valid@email.com");
}

#[test]
fn vld_text_deserializes_invalid() {
    let json = json!("not-email");
    let result: Result<VldText<EmailField>, _> = serde_json::from_value(json);
    assert!(result.is_err());
}

#[test]
fn vld_text_display() {
    let email = VldText::<EmailField>::new("a@b.com").unwrap();
    assert_eq!(format!("{}", email), "a@b.com");
}

#[test]
fn vld_text_eq() {
    let a = VldText::<EmailField>::new("a@b.com").unwrap();
    let b = VldText::<EmailField>::new("a@b.com").unwrap();
    assert_eq!(a, b);
}

// ========================= VldInt<S> =========================================

#[test]
fn vld_int_valid() {
    let age = VldInt::<AgeField>::new(25).unwrap();
    assert_eq!(*age, 25);
    assert_eq!(age.get(), 25);
}

#[test]
fn vld_int_invalid() {
    assert!(VldInt::<AgeField>::new(-1).is_err());
    assert!(VldInt::<AgeField>::new(200).is_err());
}

#[test]
fn vld_int_serializes() {
    let age = VldInt::<AgeField>::new(30).unwrap();
    let json = serde_json::to_value(&age).unwrap();
    assert_eq!(json, 30);
}

#[test]
fn vld_int_deserializes() {
    let json = json!(42);
    let age: VldInt<AgeField> = serde_json::from_value(json).unwrap();
    assert_eq!(*age, 42);
}

#[test]
fn vld_int_deserializes_invalid() {
    let json = json!(-5);
    let result: Result<VldInt<AgeField>, _> = serde_json::from_value(json);
    assert!(result.is_err());
}

// ========================= VldFloat<S> =======================================

#[test]
fn vld_float_valid() {
    let price = VldFloat::<PriceField>::new(9.99).unwrap();
    assert!((*price - 9.99).abs() < f64::EPSILON);
}

#[test]
fn vld_float_invalid() {
    assert!(VldFloat::<PriceField>::new(-0.01).is_err());
}

#[test]
fn vld_float_serializes() {
    let price = VldFloat::<PriceField>::new(19.99).unwrap();
    let json = serde_json::to_value(&price).unwrap();
    assert_eq!(json, 19.99);
}

#[test]
fn vld_float_deserializes() {
    let json = json!(42.5);
    let price: VldFloat<PriceField> = serde_json::from_value(json).unwrap();
    assert!((*price - 42.5).abs() < f64::EPSILON);
}

// ========================= VldBool<S> ========================================

#[test]
fn vld_bool_valid() {
    let active = VldBool::<ActiveField>::new(true).unwrap();
    assert_eq!(*active, true);
}

#[test]
fn vld_bool_serializes() {
    let active = VldBool::<ActiveField>::new(false).unwrap();
    let json = serde_json::to_value(&active).unwrap();
    assert_eq!(json, false);
}

#[test]
fn vld_bool_deserializes() {
    let json = json!(true);
    let active: VldBool<ActiveField> = serde_json::from_value(json).unwrap();
    assert_eq!(*active, true);
}

// ========================= Error types =======================================

#[test]
fn error_display() {
    let err = VldSurrealError::Serialization("test error".into());
    assert!(format!("{}", err).contains("test error"));
}

#[test]
fn vld_surreal_response_from_error() {
    let person = Person {
        name: "".into(),
        email: "bad".into(),
        age: -1,
    };
    let err = validate_content::<PersonSchema, _>(&person).unwrap_err();
    let response = VldSurrealResponse::from_error(&err);
    assert_eq!(response.error, "Validation failed");
    assert!(!response.fields.is_empty());
}

#[test]
fn vld_surreal_response_to_json() {
    let person = Person {
        name: "".into(),
        email: "bad".into(),
        age: -1,
    };
    let err = validate_content::<PersonSchema, _>(&person).unwrap_err();
    let response = VldSurrealResponse::from_error(&err);
    let json = response.to_json();
    assert_eq!(json["error"], "Validation failed");
    assert!(json["fields"].is_array());
}

// ========================= validate_fields! macro ============================

#[test]
fn validate_fields_macro_valid() {
    let name = "Alice";
    let email = "alice@example.com";
    let result = validate_fields! {
        name => vld::string().min(1).max(100),
        email => vld::string().email(),
    };
    assert!(result.is_ok());
}

#[test]
fn validate_fields_macro_invalid() {
    let name = "";
    let email = "not-email";
    let result = validate_fields! {
        name => vld::string().min(1),
        email => vld::string().email(),
    };
    assert!(result.is_err());
}
