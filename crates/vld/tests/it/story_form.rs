//! Stories: a form validated in the browser and a server function validating
//! its arguments with `vld::form`, the client reading the error back (the
//! way a Leptos or Dioxus app uses it).
//!
//! Compared: field names and their order, which fields fail; message texts
//! only for the summary and where a field's messages are listed.

use serde::Serialize;
use serde_json::json;
use vld::error::{PathSegment, flatten_issues, format_path};
use vld::form::*;
use vld::prelude::*;

/// Shared rules: the same functions run in the browser and on the server.
fn name_schema() -> vld::primitives::ZString {
    vld::string().min(2).max(50)
}

fn email_schema() -> vld::primitives::ZString {
    vld::string().email()
}

/// Given a sign-up form whose name input shows `check_field(&name, &name_schema())`
/// in a memo,
/// when the user types, clears and overfills it,
/// then the memo shows an error exactly while the value breaks a rule (both
/// bounds inclusive); a field with several broken rules lists all of them with
/// `check_field_all`, the first being what `check_field` shows; number
/// fields work the same.
#[test]
fn field_by_field_as_the_user_types() {
    let typed = ["", "A", "Al", "Alice", &"x".repeat(50), &"x".repeat(51)];
    let shown: Vec<bool> = typed
        .iter()
        .map(|v| check_field(&v.to_string(), &name_schema()).is_some())
        .collect();
    assert_eq!(shown, [true, true, false, false, false, true]);

    assert!(check_field(&"a@b.com".to_string(), &email_schema()).is_none());
    assert!(check_field(&"bad".to_string(), &email_schema()).is_some());

    let code = vld::string().min(5).starts_with("X");
    let all = check_field_all(&"ab".to_string(), &code);
    assert_eq!(all.len(), 2, "{all:?}");
    assert_eq!(
        check_field(&"ab".to_string(), &code).as_ref(),
        Some(&all[0])
    );
    assert_eq!(
        check_field_all(&"Xabcd".to_string(), &code),
        Vec::<String>::new()
    );
    assert_eq!(
        check_field_all(&"".to_string(), &vld::string().min(2)).len(),
        1
    );

    let age = vld::number().int().min(0).max(150);
    assert!(check_field(&25, &age).is_none());
    assert!(check_field(&150, &age).is_none());
    assert!(check_field(&-5, &age).is_some());
}

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

vld::schema! {
    struct Address {
        city: String => vld::string().min(1),
    }
}

vld::schema! {
    struct OrderSchema {
        address: Address => vld::nested(Address::parse_value),
        tags: Vec<String> => vld::array(vld::string().min(2)),
    }
}

/// Given the sign-up form bound to `UserArgs` and `UserSchema`,
/// when the user submits it valid, at the bounds, with one bad field and
/// with two,
/// then `check_all_fields` lists the bad fields in form order (empty when
/// valid); `validate` gives the same as a `VldServerError` whose summary
/// counts the fields; `validate_value` does it for a JSON form state, a
/// missing input included.
#[test]
fn whole_form_on_submit() {
    let form = |name: &str, email: &str| UserArgs {
        name: name.into(),
        email: email.into(),
    };
    let fields = |errors: Vec<FieldError>| errors.into_iter().map(|e| e.field).collect::<Vec<_>>();

    assert!(check_all_fields::<UserSchema, _>(&form("Alice", "a@b.com")).is_empty());
    for name in ["Al", &"x".repeat(50)] {
        validate::<UserSchema, _>(&form(name, "a@b.co")).unwrap();
    }

    assert_eq!(
        fields(check_all_fields::<UserSchema, _>(&form("A", "bad"))),
        ["name", "email"]
    );
    let err = validate::<UserSchema, _>(&form("A", "bad")).unwrap_err();
    assert_eq!(err.error_fields(), ["name", "email"]);
    assert_eq!(err.message, "Validation failed: 2 fields invalid");

    let err = validate::<UserSchema, _>(&form(&"x".repeat(51), "a@b.co")).unwrap_err();
    assert_eq!(err.error_fields(), ["name"]);
    assert_eq!(err.message, "Validation failed: 1 field invalid");
    let err = validate::<UserSchema, _>(&form("Alice", "bad")).unwrap_err();
    assert_eq!(err.error_fields(), ["email"]);

    let state = serde_json::json!({ "name": "Alice", "email": "a@b.com" });
    validate_value::<UserSchema>(&state).unwrap();
    let err = validate_value::<UserSchema>(&serde_json::json!({ "name": "A", "email": "a@b.com" }))
        .unwrap_err();
    assert_eq!(err.error_fields(), ["name"]);
    let err = validate_value::<UserSchema>(&serde_json::json!({ "name": "Alice" })).unwrap_err();
    assert_eq!(err.error_fields(), ["email"]);
}

/// Given an order form with a nested address and a list of tags,
/// when the city is empty and the second tag too short,
/// then the errors name the inputs as the form addresses them:
/// `address.city`, `tags[1]`.
#[test]
fn nested_and_list_inputs() {
    #[derive(Serialize)]
    struct Order {
        address: serde_json::Value,
        tags: Vec<&'static str>,
    }
    let order = Order {
        address: serde_json::json!({"city": ""}),
        tags: vec!["ok", "x"],
    };
    let errors = check_all_fields::<OrderSchema, _>(&order);
    let names: Vec<&str> = errors.iter().map(|e| e.field.as_str()).collect();
    assert_eq!(names, ["address.city", "tags[1]"]);
    assert!(errors.iter().all(|e| !e.message.is_empty()));
}

/// Form state whose serialization fails.
struct Unserializable;

impl Serialize for Unserializable {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("nope"))
    }
}

/// Given a form value that cannot be serialized,
/// when it is checked field by field, as a whole and through `validate`,
/// then it is reported as an error, never as valid: one message for the
/// field, one form error without a field name, an internal server error
/// without fields.
#[test]
fn unserializable_values_are_errors() {
    assert!(check_field(&Unserializable, &vld::string()).is_some());
    assert_eq!(check_field_all(&Unserializable, &vld::string()).len(), 1);

    let errors = check_all_fields::<UserSchema, _>(&Unserializable);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].field, "");

    let err = validate::<UserSchema, _>(&Unserializable).unwrap_err();
    assert!(err.fields.is_empty());
    assert!(err.message.contains("nope"), "{}", err.message);
}

/// Given `#[server] create_user(name, email, age)` validating its arguments
/// with `validate_args!` and the shared schemas,
/// when it is called with valid arguments (bounds inclusive), all bad, and
/// one bad,
/// then it passes, or fails naming exactly the bad arguments in order.
#[test]
fn server_function_validates_its_arguments() {
    let name = "Alice".to_string();
    let email = "alice@example.com".to_string();
    let age: i64 = 25;
    validate_args! {
        name  => name_schema(),
        email => email_schema(),
        age   => vld::number().int().min(0).max(150),
    }
    .unwrap();

    let name = "Al".to_string();
    let age: i64 = 150;
    validate_args! {
        name  => name_schema(),
        age   => vld::number().int().min(0).max(150),
    }
    .unwrap();

    let name = "A".to_string();
    let email = "bad".to_string();
    let age: i64 = -1;
    let err = validate_args! {
        name  => name_schema(),
        email => email_schema(),
        age   => vld::number().int().min(0).max(150),
    }
    .unwrap_err();
    assert_eq!(err.error_fields(), ["name", "email", "age"]);
    assert_eq!(err.message, "Validation failed: 3 fields invalid");

    let name = "Alice".to_string();
    let err = validate_args! {
        name  => name_schema(),
        email => email_schema(),
    }
    .unwrap_err();
    assert_eq!(err.error_fields(), ["email"]);

    // The server rejects what the client would reject.
    let name = "x".repeat(51);
    let email = "a@b.co".to_string();
    let err = validate_args! {
        name  => name_schema(),
        email => email_schema(),
    }
    .unwrap_err();
    assert_eq!(err.error_fields(), ["name"]);
    assert!(check_field(&name, &name_schema()).is_some());
}

/// Given a server function taking a struct argument, a list argument and a
/// code with two rules,
/// when the city is empty, the second tag too short and the code breaks both
/// rules,
/// then the errors name `address.city`, `tags[1]` and `code` twice;
/// `field_error` gives the first message of a field, `field_errors` all of
/// them in order, `error_fields` each field once.
#[test]
fn nested_arguments_and_several_errors_per_field() {
    let address = serde_json::json!({"city": ""});
    let tags = vec!["ok".to_string(), "x".to_string()];
    let code = "ab".to_string();
    let err = validate_args! {
        address => vld::nested(Address::parse_value),
        tags    => vld::array(vld::string().min(2)),
        code    => vld::string().min(5).starts_with("X"),
    }
    .unwrap_err();

    let names: Vec<&str> = err.fields.iter().map(|f| f.field.as_str()).collect();
    assert_eq!(names, ["address.city", "tags[1]", "code", "code"]);
    assert_eq!(err.error_fields(), ["address.city", "tags[1]", "code"]);
    let code_msgs = check_field_all(&code, &vld::string().min(5).starts_with("X"));
    assert_eq!(err.field_errors("code"), code_msgs);
    assert_eq!(err.field_error("code"), Some(code_msgs[0].as_str()));
    assert!(err.has_field_error("tags[1]"));
    assert!(!err.has_field_error("tags"));
    assert_eq!(err.field_error("missing"), None);
    assert!(err.field_errors("missing").is_empty());
}

/// Given the server function's error,
/// when it is sent as `ServerFnError::new(err.to_string())` and the client
/// gets the error's text (bare, or prefixed as `ServerFnError` displays it),
/// then `from_json` reads the bare text and `from_server_message` both; the
/// client sees the same fields and messages; text without an error is `None`.
#[test]
fn error_round_trips_to_the_client() {
    let name = "A".to_string();
    let email = "bad".to_string();
    let err = validate_args! {
        name  => name_schema(),
        email => email_schema(),
    }
    .unwrap_err();

    let sent = err.to_string();
    assert_eq!(serde_json::from_str::<VldServerError>(&sent).unwrap(), err);
    assert_eq!(VldServerError::from_json(&sent).unwrap(), err);

    let shown = format!("error running server function: {err}");
    assert!(VldServerError::from_json(&shown).is_none());
    let back = VldServerError::from_server_message(&shown).unwrap();
    assert_eq!(back, err);
    assert_eq!(back.field_error("email"), err.field_error("email"));

    assert!(VldServerError::from_json("not json").is_none());
    assert!(VldServerError::from_server_message("no json here").is_none());
    assert!(VldServerError::from_server_message("} {").is_none());
}

/// Given server code that builds errors itself (a uniqueness check, a
/// failure unrelated to fields, a schema's `VldError`),
/// when the client reads them,
/// then field accessors work on hand-made field lists (a field repeated
/// non-adjacently is listed once), an internal error has its message and no
/// fields, and a `VldError` converts into field errors named by path.
#[test]
fn errors_built_on_the_server() {
    let f = |field: &str, message: &str| FieldError {
        field: field.into(),
        message: message.into(),
    };
    let err = VldServerError::validation(vec![
        f("name", "too short"),
        f("email", "taken"),
        f("name", "must start with uppercase"),
    ]);
    assert_eq!(err.fields.len(), 3);
    assert_eq!(err.message, "Validation failed: 3 fields invalid");
    assert_eq!(err.field_error("name"), Some("too short"));
    assert_eq!(
        err.field_errors("name"),
        ["too short", "must start with uppercase"]
    );
    assert_eq!(err.field_error("email"), Some("taken"));
    assert_eq!(err.field_error("age"), None);
    assert!(err.has_field_error("email") && !err.has_field_error("age"));
    assert_eq!(err.error_fields(), ["name", "email"]);
    assert_eq!(VldServerError::from_json(&err.to_string()).unwrap(), err);

    let err = VldServerError::internal("something broke");
    assert_eq!(err.message, "something broke");
    assert!(err.fields.is_empty());
    assert!(err.error_fields().is_empty());

    vld::schema! {
        #[derive(Debug)]
        struct TestSchema {
            name: String => vld::string().min(5),
        }
    }
    let vld_err = TestSchema::parse_value(&serde_json::json!({ "name": "AB" })).unwrap_err();
    let msg = vld_err.issues[0].message.clone();
    let err: VldServerError = vld_err.into();
    assert_eq!(err.fields, [f("name", &msg)]);
    let root: VldServerError =
        vld::schema::VldSchema::parse_value(&vld::string().min(5), &serde_json::json!("ab"))
            .unwrap_err()
            .into();
    assert_eq!(root.error_fields(), [""]);
}

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

fn data(name: &str, email: &str, tags: &[&str]) -> OuterData {
    OuterData {
        name: name.into(),
        user: InnerData {
            email: email.into(),
        },
        tags: tags.iter().map(|t| t.to_string()).collect(),
    }
}

fn field_names(e: &VldServerError) -> Vec<&str> {
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
    assert_eq!(
        field_names(&err),
        ["name", "user.email", "tags[1]", "tags[2]"]
    );
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
    assert_eq!(field_names(&err), ["arg"]);
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
    assert_eq!(field_names(&err), ["name", "user.email", "tags[1]"]);
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
