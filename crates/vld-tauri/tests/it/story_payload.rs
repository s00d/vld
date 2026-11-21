//! Stories: `VldPayload<T>` / `VldEvent<T>` validate while Tauri deserializes
//! a command argument or an event payload.

use serde_json::json;
use vld_tauri::prelude::*;

vld::schema! {
    #[derive(Debug, Clone, PartialEq)]
    pub struct CreateUser {
        pub name: String  => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}

vld::schema! {
    #[derive(Debug, Clone, PartialEq)]
    pub struct UserUpdate {
        pub id: i64 => vld::number().int().min(1),
        pub name: String => vld::string().min(2),
    }
}

/// Given a command declared as `fn create_user(payload: VldPayload<CreateUser>)`,
/// when Tauri deserializes a valid argument,
/// then the command reads the fields through `Deref`, can change them through
/// `DerefMut`, clone and debug-print the wrapper.
#[test]
fn command_argument_is_validated_on_deserialize() {
    let mut payload: VldPayload<CreateUser> =
        serde_json::from_value(json!({"name": "Alice", "email": "alice@example.com"})).unwrap();
    assert_eq!(payload.name, "Alice");
    let user: &CreateUser = &payload;
    assert_eq!(user.email, "alice@example.com");

    payload.name.push_str(" B.");
    assert_eq!(payload.0.name, "Alice B.");

    let copy = payload.clone();
    assert_eq!(copy.0, payload.0);
    assert_eq!(format!("{copy:?}"), format!("VldPayload({:?})", payload.0),);

    // From a JSON string, as Tauri hands it over.
    let p: VldPayload<CreateUser> =
        serde_json::from_str(r#"{"name":"Bob","email":"bob@test.com"}"#).unwrap();
    assert_eq!(p.name, "Bob");
}

/// Given the same command,
/// when the argument fails validation,
/// then Tauri gets a deserialization error whose text lists every issue as
/// `path: message` (no leading dot, no empty prefix for a root issue),
/// joined by `; ` after `Validation failed: `.
#[test]
fn invalid_argument_becomes_one_error_string() {
    let msg =
        serde_json::from_value::<VldPayload<CreateUser>>(json!({"name": "A", "email": "a@b.co"}))
            .unwrap_err()
            .to_string();
    let expected = validate::<CreateUser>(json!({"name": "A", "email": "a@b.co"})).unwrap_err();
    assert_eq!(
        msg,
        format!("Validation failed: name: {}", expected.issues[0].message)
    );

    let msg =
        serde_json::from_value::<VldPayload<CreateUser>>(json!({"name": "A", "email": "bad"}))
            .unwrap_err()
            .to_string();
    let e = validate::<CreateUser>(json!({"name": "A", "email": "bad"})).unwrap_err();
    assert_eq!(
        msg,
        format!(
            "Validation failed: name: {}; email: {}",
            e.issues[0].message, e.issues[1].message
        )
    );

    // A root issue has no `: ` prefix.
    let msg = serde_json::from_value::<VldPayload<CreateUser>>(json!(42))
        .unwrap_err()
        .to_string();
    let e = validate::<CreateUser>(json!(42)).unwrap_err();
    assert_eq!(msg, format!("Validation failed: {}", e.issues[0].message));

    // Not even JSON: serde's own error, before validation.
    let err = serde_json::from_str::<VldPayload<CreateUser>>("{").unwrap_err();
    assert!(err.is_eof(), "{err}");
}

/// Given a backend listener that reads `user:update` events as
/// `VldEvent<UserUpdate>`,
/// when the frontend emits a valid and an invalid payload,
/// then the valid one is a `VldPayload` with the fields, the invalid one is a
/// deserialization error naming both bad fields.
#[test]
fn event_payload_is_validated_on_deserialize() {
    let event: VldEvent<UserUpdate> =
        serde_json::from_str(r#"{"id": 5, "name": "Carol"}"#).unwrap();
    assert_eq!((event.id, event.name.as_str()), (5, "Carol"));
    let inner: &UserUpdate = &event;
    assert_eq!(inner.id, 5);
    // VldEvent is VldPayload.
    let VldPayload(update) = event;
    assert_eq!(update.name, "Carol");

    let msg = serde_json::from_value::<VldEvent<UserUpdate>>(json!({"id": 0, "name": "X"}))
        .unwrap_err()
        .to_string();
    assert!(msg.starts_with("Validation failed: id: "), "{msg}");
    assert!(msg.contains("; name: "), "{msg}");

    // The same payload through `validate`: structured issues.
    let err = validate::<UserUpdate>(json!({"id": 0, "name": "X"})).unwrap_err();
    assert_eq!(err.issue_count(), 2);
}
