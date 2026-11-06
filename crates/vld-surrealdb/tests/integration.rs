use serde_json::json;
#[allow(unused_imports)]
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

/// `(path, code)` of every issue of a `Validation` error (panics on `Ok`).
#[track_caller]
fn issues<T: std::fmt::Debug>(r: Result<T, VldSurrealError>) -> Vec<(String, String)> {
    match r {
        Err(VldSurrealError::Validation(e)) => e
            .issues
            .iter()
            .map(|i| {
                (
                    i.path.iter().map(ToString::to_string).collect(),
                    i.code.key().to_string(),
                )
            })
            .collect(),
        other => panic!("expected a validation error, got {other:?}"),
    }
}

/// Owned `(path, code)` pairs for comparing with [`issues`].
fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
    expected
        .iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect()
}

// ========================= Test data =========================================

#[derive(Debug, serde::Serialize, serde::Deserialize)]
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
    validate_content::<PersonSchema, _>(&person).unwrap();
    // Bounds are inclusive.
    let edge = Person {
        name: "A".into(),
        email: "a@b.co".into(),
        age: 150,
    };
    validate_content::<PersonSchema, _>(&edge).unwrap();
}

#[test]
fn validate_content_invalid_name() {
    let person = Person {
        name: "".into(),
        email: "alice@example.com".into(),
        age: 30,
    };
    assert_eq!(
        issues(validate_content::<PersonSchema, _>(&person)),
        pairs(&[(".name", "too_small")])
    );
}

#[test]
fn validate_content_invalid_email() {
    let person = Person {
        name: "Alice".into(),
        email: "not-an-email".into(),
        age: 30,
    };
    assert_eq!(
        issues(validate_content::<PersonSchema, _>(&person)),
        pairs(&[(".email", "invalid_string")])
    );
}

#[test]
fn validate_content_invalid_age() {
    let person = Person {
        name: "Alice".into(),
        email: "alice@example.com".into(),
        age: -1,
    };
    assert_eq!(
        issues(validate_content::<PersonSchema, _>(&person)),
        pairs(&[(".age", "too_small")])
    );
}

// ========================= validate_json =====================================

#[test]
fn validate_json_valid() {
    let json = json!({"name": "Bob", "email": "bob@example.com", "age": 25});
    validate_json::<PersonSchema>(&json).unwrap();
}

#[test]
fn validate_json_invalid() {
    let json = json!({"name": "", "email": "bad", "age": 200});
    assert_eq!(
        issues(validate_json::<PersonSchema>(&json)),
        pairs(&[
            (".name", "too_small"),
            (".email", "invalid_string"),
            (".age", "too_big")
        ])
    );
}

#[test]
fn validate_json_missing_fields() {
    let json = json!({"name": "Bob"});
    assert_eq!(
        issues(validate_json::<PersonSchema>(&json)),
        pairs(&[(".email", "missing_field"), (".age", "missing_field")])
    );
}

// ========================= validate_record(s) ================================

#[test]
fn validate_record_valid() {
    let p = Person {
        name: "Charlie".into(),
        email: "charlie@example.com".into(),
        age: 40,
    };
    #[allow(deprecated)]
    let r = validate_record::<PersonSchema, _>(&p);
    r.unwrap();
    let bad = Person { age: 151, ..p };
    #[allow(deprecated)]
    let r = validate_record::<PersonSchema, _>(&bad);
    assert_eq!(issues(r), pairs(&[(".age", "too_big")]));
}

#[test]
fn validate_records_all_valid() {
    let rows = vec![
        NameOnly {
            name: "Alice".into(),
        },
        NameOnly { name: "Bob".into() },
    ];
    validate_records::<NameSchema, _>(&rows).unwrap();
    validate_records::<NameSchema, NameOnly>(&[]).unwrap();
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
    assert_eq!(issues::<()>(Err(err.1)), pairs(&[(".name", "too_small")]));
}

// ========================= validate_value ====================================

#[test]
fn validate_value_valid_string() {
    let schema = vld::string().min(1).max(100);
    validate_value(&schema, &json!("Hello")).unwrap();
    validate_value(&schema, &json!("H")).unwrap();
    assert_eq!(
        issues(validate_value(&schema, &json!("x".repeat(101)))),
        pairs(&[("", "too_big")])
    );
}

#[test]
fn validate_value_invalid_string() {
    let schema = vld::string().min(5);
    assert_eq!(
        issues(validate_value(&schema, &json!("Hi"))),
        pairs(&[("", "too_small")])
    );
    assert_eq!(
        issues(validate_value(&schema, &json!(5))),
        pairs(&[("", "invalid_type")])
    );
}

#[test]
fn validate_value_valid_number() {
    let schema = vld::number().int().min(0).max(100);
    for ok in [0, 42, 100] {
        validate_value(&schema, &json!(ok)).unwrap();
    }
    assert_eq!(
        issues(validate_value(&schema, &json!(1.5))),
        pairs(&[("", "not_int")])
    );
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
    assert_eq!(
        issues(Validated::<PersonSchema, _>::new(p)),
        pairs(&[
            (".name", "too_small"),
            (".email", "invalid_string"),
            (".age", "too_small")
        ])
    );
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
    assert_eq!(
        issues(VldText::<EmailField>::new("not-email")),
        pairs(&[(".value", "invalid_string")])
    );
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
    let err = serde_json::from_value::<VldText<EmailField>>(json).unwrap_err();
    assert!(
        err.to_string().starts_with("Validation error: .value: "),
        "{err}"
    );
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
    assert_eq!(
        issues(VldInt::<AgeField>::new(-1)),
        pairs(&[(".value", "too_small")])
    );
    assert_eq!(
        issues(VldInt::<AgeField>::new(200)),
        pairs(&[(".value", "too_big")])
    );
}

#[test]
fn vld_int_serializes() {
    let age = VldInt::<AgeField>::new(30).unwrap();
    let json = serde_json::to_value(age).unwrap();
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
    let err = serde_json::from_value::<VldInt<AgeField>>(json).unwrap_err();
    assert!(
        err.to_string().starts_with("Validation error: .value: "),
        "{err}"
    );
}

// ========================= VldFloat<S> =======================================

#[test]
fn vld_float_valid() {
    let price = VldFloat::<PriceField>::new(9.99).unwrap();
    assert!((*price - 9.99).abs() < f64::EPSILON);
}

#[test]
fn vld_float_invalid() {
    assert_eq!(
        issues(VldFloat::<PriceField>::new(-0.01)),
        pairs(&[(".value", "too_small")])
    );
}

#[test]
fn vld_float_serializes() {
    let price = VldFloat::<PriceField>::new(19.99).unwrap();
    let json = serde_json::to_value(price).unwrap();
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
    assert!(*active);
}

#[test]
fn vld_bool_serializes() {
    let active = VldBool::<ActiveField>::new(false).unwrap();
    let json = serde_json::to_value(active).unwrap();
    assert_eq!(json, false);
}

#[test]
fn vld_bool_deserializes() {
    let json = json!(true);
    let active: VldBool<ActiveField> = serde_json::from_value(json).unwrap();
    assert!(*active);
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
    result.unwrap();
}

#[test]
fn validate_fields_macro_invalid() {
    let name = "";
    let email = "not-email";
    let result = validate_fields! {
        name => vld::string().min(1),
        email => vld::string().email(),
    };
    assert_eq!(
        issues(result),
        pairs(&[(".name", "too_small"), (".email", "invalid_string")])
    );
}

// ========================= Regressions =======================================

vld::schema! {
    #[derive(Debug)]
    pub struct InnerAddr {
        pub city: String => vld::string().min(2),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct OuterDoc {
        pub tags: Vec<String> => vld::array(vld::string().min(1)),
        pub addr: InnerAddr => vld::nested(InnerAddr::parse_value),
    }
}

#[test]
fn response_field_paths_have_no_leading_dot() {
    let json = serde_json::json!({"tags": [""], "addr": {"city": "X"}});
    let err = vld_surrealdb::validate_json::<OuterDoc>(&json).unwrap_err();
    let resp = VldSurrealResponse::from_error(&err);
    let fields: Vec<&str> = resp.fields.iter().map(|f| f.field.as_str()).collect();
    assert!(fields.contains(&"tags[0]"), "{fields:?}");
    assert!(fields.contains(&"addr.city"), "{fields:?}");
}

#[test]
fn validate_fields_keeps_real_issues() {
    let name = "";
    let err = validate_fields! { name => vld::string().min(3) }.unwrap_err();
    let VldSurrealError::Validation(e) = err else {
        panic!("expected validation error")
    };
    let issue = &e.issues[0];
    assert_ne!(issue.message, "field 'name' failed validation");
    assert!(!matches!(issue.code, vld::error::IssueCode::Custom { .. }));
    assert!(matches!(
        issue.path.as_slice(),
        [vld::error::PathSegment::Field(f)] if f == "name"
    ));
}

struct Unserializable;
impl serde::Serialize for Unserializable {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("nope"))
    }
}

#[test]
fn validate_fields_reports_serialization_failure() {
    let bad = Unserializable;
    let err = validate_fields! { bad => vld::any() }.unwrap_err();
    assert!(matches!(err, VldSurrealError::Serialization(_)));
}
