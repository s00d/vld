//! Stories: a server function validates its arguments; the client reads the
//! error back.

use vld_leptos::prelude::*;

fn name_schema() -> vld::primitives::ZString {
    vld::string().min(2).max(50)
}

fn email_schema() -> vld::primitives::ZString {
    vld::string().email()
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
    vld_leptos::validate_args! {
        name  => name_schema(),
        email => email_schema(),
        age   => vld::number().int().min(0).max(150),
    }
    .unwrap();

    let name = "Al".to_string();
    let age: i64 = 150;
    vld_leptos::validate_args! {
        name  => name_schema(),
        age   => vld::number().int().min(0).max(150),
    }
    .unwrap();

    let name = "A".to_string();
    let email = "bad".to_string();
    let age: i64 = -1;
    let err = vld_leptos::validate_args! {
        name  => name_schema(),
        email => email_schema(),
        age   => vld::number().int().min(0).max(150),
    }
    .unwrap_err();
    assert_eq!(err.error_fields(), ["name", "email", "age"]);
    assert_eq!(err.message, "Validation failed: 3 fields invalid");

    let name = "Alice".to_string();
    let err = vld_leptos::validate_args! {
        name  => name_schema(),
        email => email_schema(),
    }
    .unwrap_err();
    assert_eq!(err.error_fields(), ["email"]);

    // The server rejects what the client would reject.
    let name = "x".repeat(51);
    let email = "a@b.co".to_string();
    let err = vld_leptos::validate_args! {
        name  => name_schema(),
        email => email_schema(),
    }
    .unwrap_err();
    assert_eq!(err.error_fields(), ["name"]);
    assert!(check_field(&name, &name_schema()).is_some());
}

vld::schema! {
    struct Address {
        city: String => vld::string().min(1),
    }
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
    let err = vld_leptos::validate_args! {
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
    let err = vld_leptos::validate_args! {
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
