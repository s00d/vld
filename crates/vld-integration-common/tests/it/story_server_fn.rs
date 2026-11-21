//! Stories of a Leptos / Dioxus server function validating its input with
//! the `form` helpers and the client reading the error back.

use serde::Serialize;
use serde_json::json;
use vld::error::PathSegment;
use vld::schema::VldSchema;
use vld_integration_common::form::*;
use vld_integration_common::{flatten_issues, format_path, validate_args};

vld::schema! {
    #[derive(Debug)]
    pub struct Inner {
        pub email: String => vld::string().email(),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct Outer {
        pub name: String => vld::string().min(2),
        pub user: Inner => vld::nested(Inner::parse_value),
        pub tags: Vec<String> => vld::array(vld::string().min(2)),
    }
}

#[derive(Serialize)]
struct OuterData {
    name: String,
    user: InnerData,
    tags: Vec<String>,
}

#[derive(Serialize)]
struct InnerData {
    email: String,
}

/// A value whose serialization always fails.
struct Unserializable;
impl Serialize for Unserializable {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("nope"))
    }
}

fn data(name: &str, email: &str, tags: &[&str]) -> OuterData {
    OuterData {
        name: name.into(),
        user: InnerData {
            email: email.into(),
        },
        tags: tags.iter().map(|t| t.to_string()).collect(),
    }
}

fn fields(e: &VldServerError) -> Vec<&str> {
    e.fields.iter().map(|f| f.field.as_str()).collect()
}

/// Given a server function `register(name, user, tags)` that checks its
/// arguments with `validate_args!`,
/// when it is called with bad arguments and the error travels to the client
/// inside a framework message,
/// then every bad argument is reported under its argument path, and the
/// client reads the same error back.
#[test]
fn server_function_arguments_round_trip_to_the_client() {
    fn register(
        name: String,
        user: serde_json::Value,
        tags: Vec<String>,
    ) -> Result<(), VldServerError> {
        validate_args! {
            name => vld::string().min(2),
            user => vld::nested(Inner::parse_value),
            tags => vld::array(vld::string().min(2)),
        }
    }

    // Valid call.
    assert_eq!(
        register("Al".into(), json!({"email": "a@b.co"}), vec!["ok".into()]),
        Ok(())
    );

    // Root, nested and indexed issues get argument paths.
    let err = register(
        "A".into(),
        json!({"email": "bad"}),
        vec!["ok".into(), "x".into(), "y".into()],
    )
    .unwrap_err();
    assert_eq!(fields(&err), ["name", "user.email", "tags[1]", "tags[2]"]);
    assert_eq!(err.message, "Validation failed: 4 fields invalid");
    assert!(err.has_field_error("tags[1]"));
    assert!(!err.has_field_error("tags"));
    assert_eq!(err.field_error("missing"), None);

    // The client gets the error wrapped in a framework message.
    let shown = format!("error running server function: {err} (status 500)");
    let back = VldServerError::from_server_message(&shown).unwrap();
    assert_eq!(back, err);
    assert_eq!(
        VldServerError::from_json(&err.to_string()),
        Some(err.clone())
    );
    assert_eq!(VldServerError::from_server_message("no json here"), None);
    assert_eq!(VldServerError::from_server_message("} reversed {"), None);
    assert_eq!(VldServerError::from_json("{\"message\": 1}"), None);

    // One bad argument: singular summary.
    let one = register("Al".into(), json!({"email": "bad"}), vec![]).unwrap_err();
    assert_eq!(one.message, "Validation failed: 1 field invalid");

    // An argument that cannot be serialized is an error under its name.
    let arg = Unserializable;
    let err = validate_args! { arg => vld::string() }.unwrap_err();
    assert_eq!(fields(&err), ["arg"]);
    assert!(err.fields[0].message.starts_with("Serialization error: "));
}

/// Given a request struct checked as a whole against a schema type,
/// when a client fills it in wrong,
/// then `validate`, `validate_value`, `check_all_fields` and the
/// `From<VldError>` conversion report the same dotted field paths, and the
/// per-field lookups find them.
#[test]
fn a_struct_is_checked_as_a_whole_and_field_by_field() {
    let good = data("Alice", "a@b.co", &["rust"]);
    assert_eq!(validate::<Outer, _>(&good), Ok(()));
    assert_eq!(
        validate_value::<Outer>(&serde_json::to_value(&good).unwrap()),
        Ok(())
    );
    assert!(check_all_fields::<Outer, _>(&good).is_empty());

    let bad = data("A", "bad", &["ok", "x"]);
    let err = validate::<Outer, _>(&bad).unwrap_err();
    assert_eq!(fields(&err), ["name", "user.email", "tags[1]"]);
    assert_eq!(
        validate_value::<Outer>(&serde_json::to_value(&bad).unwrap()).unwrap_err(),
        err
    );
    assert_eq!(check_all_fields::<Outer, _>(&bad), err.fields);
    let from: VldServerError = Outer::parse_value(&serde_json::to_value(&bad).unwrap())
        .unwrap_err()
        .into();
    assert_eq!(from, err);

    // The same paths from the raw error.
    let raw = Outer::parse_value(&serde_json::to_value(&bad).unwrap()).unwrap_err();
    let flat: Vec<String> = flatten_issues(&raw).into_iter().map(|(p, _)| p).collect();
    assert_eq!(flat, ["name", "user.email", "tags[1]"]);
    assert_eq!(flatten_issues(&raw)[0].1, raw.issues[0].message);
    assert_eq!(
        format_path(&[
            PathSegment::Field("items".into()),
            PathSegment::Index(0),
            PathSegment::Field("name".into())
        ]),
        "items[0].name"
    );
    assert_eq!(
        format_path(&[PathSegment::Index(2), PathSegment::Index(3)]),
        "[2][3]"
    );
    assert_eq!(format_path(&[]), "");

    // Lookups: first message, all messages, unique fields in first-seen order.
    let f = |field: &str, message: &str| FieldError {
        field: field.into(),
        message: message.into(),
    };
    let e = VldServerError::validation(vec![f("a", "1"), f("b", "2"), f("a", "3")]);
    assert_eq!(e.field_error("a"), Some("1"));
    assert_eq!(e.field_errors("a"), ["1", "3"]);
    assert!(e.field_errors("c").is_empty());
    assert_eq!(e.error_fields(), ["a", "b"]);
    assert_eq!(
        VldServerError::validation(vec![]).message,
        "Validation failed: 0 fields invalid"
    );

    // Internal errors carry no fields.
    let internal = VldServerError::internal("db down");
    assert_eq!(
        (internal.message.as_str(), internal.fields.len()),
        ("db down", 0)
    );
    assert_eq!(internal.to_string(), r#"{"message":"db down","fields":[]}"#);

    // Serialization failures are errors, never "valid".
    let v = validate::<Outer, _>(&Unserializable).unwrap_err();
    assert!(v.message.starts_with("Serialization error: ") && v.fields.is_empty());
    let all = check_all_fields::<Outer, _>(&Unserializable);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].field, "");
    assert!(all[0].message.starts_with("Serialization error: "));
}

/// Given form inputs checked one field at a time as the user types,
/// when a value is valid, has one problem or several,
/// then `check_field` gives the first message and `check_field_all` every
/// message; an unserializable value is an error.
#[test]
fn single_fields_are_checked_as_the_user_types() {
    let schema = vld::string().min(5).email();
    assert_eq!(check_field(&"alice@example.com", &schema), None);
    assert!(check_field_all(&"alice@example.com", &schema).is_empty());

    let all = check_field_all(&"ab", &schema);
    assert_eq!(all.len(), 2);
    assert_eq!(check_field(&"ab", &schema), Some(all[0].clone()));
    let expected: Vec<String> = schema
        .parse_value(&json!("ab"))
        .unwrap_err()
        .issues
        .iter()
        .map(|i| i.message.clone())
        .collect();
    assert_eq!(all, expected);

    assert!(
        check_field(&Unserializable, &vld::string())
            .unwrap()
            .starts_with("Serialization error: ")
    );
    assert_eq!(check_field_all(&Unserializable, &vld::string()).len(), 1);
}
