//! Stories: a form validated in the browser.

use serde::Serialize;
use vld_leptos::prelude::*;

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
