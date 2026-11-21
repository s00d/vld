//! Stories: whole rows validated before Diesel writes them and after it loads
//! them (`validate_insert`, `Validated`, `validate_row`).

use super::common::*;
use diesel::prelude::*;
use vld_data::diesel::prelude::*;

/// Given a valid user and a `users` table,
/// when it is wrapped in `Validated` and inserted with `.values(v.inner())`,
/// then it loads back unchanged; `Deref`, `Clone`, `Debug`, `Serialize` and
/// `into_inner` expose the original value.
#[test]
fn validated_user_is_inserted_and_loaded() {
    let mut conn = db(&[USERS]);
    let validated =
        Validated::<UserSchema, _>::new(user("Alice", "alice@example.com", 30)).unwrap();
    assert_eq!(validated.name, "Alice");
    assert_eq!(
        serde_json::to_value(&validated).unwrap(),
        serde_json::json!({"name": "Alice", "email": "alice@example.com", "age": 30})
    );
    assert!(format!("{validated:?}").starts_with("Validated { inner: NewUser"));

    diesel::insert_into(users::table)
        .values(validated.inner())
        .execute(&mut conn)
        .unwrap();
    let loaded: Vec<User> = users::table
        .select(User::as_select())
        .load(&mut conn)
        .unwrap();
    assert_eq!(loaded.len(), 1);
    let back = user(&loaded[0].name, &loaded[0].email, loaded[0].age);
    assert_eq!(back, validated.clone().into_inner());
}

/// Given users that break the schema in one or several fields,
/// when each is checked with `validate_insert` and `Validated::new`,
/// then the issues name every bad field with its code and nothing is
/// inserted; inclusive bounds pass and are inserted.
#[test]
fn invalid_users_are_rejected_before_insert() {
    let mut conn = db(&[USERS]);
    let cases = [
        (
            user("Bob", "not-an-email", 25),
            vec![(".email", "invalid_string")],
        ),
        (
            user("Charlie", "c@example.com", -5),
            vec![(".age", "too_small")],
        ),
        (user("Dan", "d@example.com", 151), vec![(".age", "too_big")]),
        (user("", "x@y.com", 20), vec![(".name", "too_small")]),
        (
            user("", "bad", -1),
            vec![
                (".name", "too_small"),
                (".email", "invalid_string"),
                (".age", "too_small"),
            ],
        ),
    ];
    for (bad, expected) in cases {
        assert_eq!(
            issues(validate_insert::<UserSchema, _>(&bad)),
            pairs(&expected)
        );
        assert_eq!(
            issues(Validated::<UserSchema, _>::new(bad)),
            pairs(&expected)
        );
    }
    for edge in [user("A", "a@b.co", 0), user("Z", "z@b.co", 150)] {
        validate_insert::<UserSchema, _>(&edge).unwrap();
        diesel::insert_into(users::table)
            .values(&edge)
            .execute(&mut conn)
            .unwrap();
    }
    let count: i64 = users::table.count().get_result(&mut conn).unwrap();
    assert_eq!(count, 2);
}

/// Given legacy rows inserted with raw SQL (one broken),
/// when they are loaded as `User` and re-checked with `validate_row`,
/// then the good row passes and the broken one names its field.
#[test]
fn loaded_rows_are_rechecked() {
    let mut conn = db(&[USERS]);
    diesel::sql_query(
        "INSERT INTO users (name, email, age) VALUES ('Test', 'test@x.com', 20), ('Nope', 'nope', 20)",
    )
    .execute(&mut conn)
    .unwrap();
    let loaded: Vec<User> = users::table
        .order(users::id)
        .select(User::as_select())
        .load(&mut conn)
        .unwrap();
    assert_eq!(loaded[0].name, "Test");
    validate_row::<UserSchema, _>(&loaded[0]).unwrap();
    assert_eq!(
        issues(validate_row::<UserSchema, _>(&loaded[1])),
        pairs(&[(".email", "invalid_string")])
    );
}

/// A value whose `Serialize` impl fails.
struct Unserializable;
impl serde::Serialize for Unserializable {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("nope"))
    }
}

/// Given a value that cannot be serialized and a raw `VldError`,
/// when they become `VldDieselError`s,
/// then the first is `Serialization` (text carries the cause, no source) and
/// the second `Validation` (source is the `VldError`).
#[test]
fn error_kinds() {
    use std::error::Error as _;
    let err = validate_insert::<UserSchema, _>(&Unserializable).unwrap_err();
    assert!(
        matches!(&err, VldDieselError::Serialization(m) if m == "nope"),
        "{err:?}"
    );
    assert_eq!(err.to_string(), "Serialization error: nope");
    assert!(err.source().is_none());

    let vld_err = vld::error::VldError::single(vld::error::IssueCode::MissingField, "required");
    let err: VldDieselError = vld_err.into();
    assert_eq!(issues_of(&err), pairs(&[("", "missing_field")]));
    assert!(err.to_string().starts_with("Validation error: "));
    assert!(err.source().is_some());
}
