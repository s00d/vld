//! `schema!` / `schema_validated!`: parsing, nesting, renames, `validate`.

use crate::common::{codes, paths};
use serde_json::json;
use vld::error::IssueCode;
use vld::prelude::*;

vld::schema! {
    #[derive(Debug, serde::Serialize)]
    struct User {
        name: String => vld::string().min(2).max(50),
        email: String => vld::string().email(),
        age: Option<i64> => vld::number().int().min(18).optional(),
    }
}

#[test]
fn parses_typed_struct() {
    let u = User::parse(r#"{"name": "Alex", "email": "a@b.co", "age": 25}"#).unwrap();
    assert_eq!((u.name.as_str(), u.age), ("Alex", Some(25)));
    let u = User::parse(&json!({"name": "Bob", "email": "b@c.co"})).unwrap();
    assert_eq!(u.age, None);
}

#[test]
fn accumulates_errors_with_paths() {
    let err = User::parse(r#"{"name": "A", "email": "bad", "age": 5}"#).unwrap_err();
    assert_eq!(paths(&err), [".name", ".email", ".age"]);
    assert_eq!(codes(&err), ["too_small", "invalid_string", "too_small"]);
    assert_issue!(User::parse("[1]"), "", IssueCode::InvalidType { expected, .. } if expected == "object");
}

#[test]
fn nested_struct_paths() {
    vld::schema! {
        #[derive(Debug)]
        struct Address {
            city: String => vld::string().min(1),
        }
    }
    vld::schema! {
        #[derive(Debug)]
        struct Person {
            name: String => vld::string(),
            address: Address => vld::nested(Address::parse_value),
        }
    }
    let p = Person::parse(r#"{"name": "Alex", "address": {"city": "Moscow"}}"#).unwrap();
    assert_eq!(p.address.city, "Moscow");
    let err = Person::parse(r#"{"name": "Alex", "address": {"city": ""}}"#).unwrap_err();
    assert_eq!(paths(&err), [".address.city"]);
}

#[test]
fn enum_field() {
    vld::schema! {
        #[derive(Debug)]
        struct Config {
            mode: String => vld::enumeration(&["dev", "prod"]),
        }
    }
    assert_eq!(Config::parse(r#"{"mode": "dev"}"#).unwrap().mode, "dev");
    assert_issue!(
        Config::parse(r#"{"mode": "x"}"#),
        ".mode",
        IssueCode::InvalidEnumValue { .. }
    );
}

// --- renames ----------------------------------------------------------------

vld::schema! {
    #[derive(Debug, PartialEq)]
    pub struct RenamedUser {
        pub first_name: String as "firstName" => vld::string().min(1),
        pub last_name: String as "lastName" => vld::string().min(1),
        pub email_address: String => vld::string().email(),
    }
}

#[test]
fn rename_reads_and_reports_the_json_key() {
    let u = RenamedUser::parse(
        r#"{"firstName": "John", "lastName": "Doe", "email_address": "j@d.co"}"#,
    )
    .unwrap();
    assert_eq!(
        (u.first_name.as_str(), u.last_name.as_str()),
        ("John", "Doe")
    );

    // The Rust names are not read.
    let err = RenamedUser::parse(
        r#"{"first_name": "John", "last_name": "Doe", "email_address": "j@d.co"}"#,
    )
    .unwrap_err();
    assert_eq!(paths(&err), [".firstName", ".lastName"]);

    let u = <RenamedUser as VldParse>::vld_parse_value(&json!({
        "firstName": "Jane", "lastName": "S", "email_address": "j@s.co"
    }))
    .unwrap();
    assert_eq!(u.first_name, "Jane");
}

vld::schema! {
    #[derive(Debug)]
    pub struct RawIdent {
        pub r#type: String => vld::string(),
    }
}

#[test]
fn raw_identifier_prefix_is_stripped() {
    assert_eq!(
        RawIdent::parse_value(&json!({"type": "a"})).unwrap().r#type,
        "a"
    );
    assert_eq!(
        paths(&RawIdent::parse_value(&json!({})).unwrap_err()),
        [".type"]
    );
}

#[cfg(feature = "openapi")]
#[test]
fn json_schema_uses_renamed_keys() {
    let js = RenamedUser::json_schema();
    let mut props: Vec<&String> = js["properties"].as_object().unwrap().keys().collect();
    props.sort();
    assert_eq!(props, ["email_address", "firstName", "lastName"]);
    assert_eq!(
        js["required"],
        json!(["firstName", "lastName", "email_address"])
    );
}

// --- validate / is_valid on instances ----------------------------------------

#[test]
fn validate_existing_instances_and_values() {
    let good = User::parse(r#"{"name": "Alice", "email": "a@b.com", "age": 25}"#).unwrap();
    assert!(User::is_valid(&good));
    assert!(User::validate(&good).is_ok());

    let bad = User {
        name: "A".into(),
        email: "bad".into(),
        age: Some(10),
    };
    assert!(!User::is_valid(&bad));
    assert_eq!(
        paths(&User::validate(&bad).unwrap_err()),
        [".name", ".email", ".age"]
    );
    assert!(User::is_valid(&json!({"name": "Bob", "email": "b@t.co"})));
}

#[test]
fn schema_validated_and_into_params_attribute_are_accepted() {
    vld::schema! {
        #[derive(Debug)]
        #[into_params(parameter_in = Query)]
        struct Probe {
            x: i64 => vld::number().int(),
        }
    }
    assert_eq!(Probe::parse(r#"{"x": 1}"#).unwrap().x, 1);

    vld::schema_validated! {
        #[derive(Debug, Default, serde::Serialize)]
        #[into_params(parameter_in = Query)]
        pub struct ValidatedRenamed {
            pub full_name: String as "fullName" => vld::string().min(1),
            pub score: f64 => vld::number().min(0.0).max(100.0),
        }
    }
    let json = r#"{"fullName": "Bob", "score": 95.5}"#;
    assert_eq!(ValidatedRenamed::parse(json).unwrap().full_name, "Bob");
    assert!(ValidatedRenamed::parse_lenient(json).unwrap().is_valid());
    let lenient = ValidatedRenamed::parse_lenient(r#"{"fullName": "", "score": 101}"#).unwrap();
    assert_eq!(lenient.error_count(), 2);
}

// ---- parse_coerced: values coerced from text follow the schema ----

vld::schema! {
    #[derive(Debug)]
    struct Query {
        q: String => vld::string(),
        age: i64 => vld::number().int(),
        zip: String => vld::string(),
        tags: Vec<String> => vld::array(vld::string()),
        ids: Vec<String> => vld::array(vld::string()),
        flag: String => vld::string(),
    }
}

#[test]
fn parse_coerced_follows_schema() {
    let v = json!({"q": 123, "age": 30, "zip": "01234", "tags": "a", "ids": 7, "flag": true});
    let q: Query = vld::schema::parse_coerced(&v).unwrap();
    assert_eq!(q.q, "123");
    assert_eq!(q.age, 30);
    assert_eq!(q.zip, "01234");
    assert_eq!(q.tags, ["a"]);
    assert_eq!(q.ids, ["7"]);
    assert_eq!(q.flag, "true");
}

#[test]
fn parse_coerced_keeps_real_errors() {
    let v = json!({"q": 1, "age": "abc", "zip": "1", "tags": [], "ids": [], "flag": "x"});
    let err = vld::schema::parse_coerced::<Query>(&v).unwrap_err();
    assert_eq!(err.issues.len(), 1);
    assert!(matches!(err.issues[0].code, IssueCode::InvalidType { .. }));
}
