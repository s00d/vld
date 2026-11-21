//! Stories: whole rows validated before they are written with SQLx and after
//! they are read back (`validate_insert`, `Validated`, `validate_row`,
//! `validate_rows`).

use super::common::*;
use super::sqlx::{self, Row};
use super::vld_sqlx::prelude::*;

/// Given a valid user and a `users` table,
/// when it is wrapped in `Validated` and its fields are bound into an INSERT,
/// then the row reads back unchanged; `Deref`, `inner`, `into_inner`, `Clone`,
/// `Debug` and `Serialize` all expose the original value.
#[tokio::test]
async fn validated_user_is_inserted_and_read_back() {
    let pool = users_db().await;
    let validated =
        Validated::<UserSchema, _>::new(user("Alice", "alice@example.com", 30)).unwrap();
    assert_eq!(validated.name, "Alice"); // Deref
    assert_eq!(validated.inner().age, 30);
    assert_eq!(
        serde_json::to_value(&validated).unwrap(),
        serde_json::json!({"name": "Alice", "email": "alice@example.com", "age": 30})
    );
    assert!(format!("{validated:?}").starts_with("Validated { inner: NewUser"));
    let copy = validated.clone();

    sqlx::query("INSERT INTO users (name, email, age) VALUES (?, ?, ?)")
        .bind(&validated.name)
        .bind(&validated.email)
        .bind(validated.age)
        .execute(&pool)
        .await
        .unwrap();

    let row = sqlx::query("SELECT name, email, age FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    let back = user(row.get("name"), row.get("email"), row.get("age"));
    assert_eq!(back, copy.into_inner());
}

/// Given users that break the schema in one or several fields,
/// when each is checked with `validate_insert` and `Validated::new`,
/// then nothing is written and the issues name every bad field with its code;
/// inclusive bounds pass.
#[tokio::test]
async fn invalid_users_are_rejected_before_insert() {
    let pool = users_db().await;
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
            user(&"n".repeat(101), "x@y.com", 20),
            vec![(".name", "too_big")],
        ),
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
    for edge in [
        user("A", "a@b.co", 0),
        user(&"n".repeat(100), "a@b.co", 150),
    ] {
        validate_insert::<UserSchema, _>(&edge).unwrap();
        sqlx::query("INSERT INTO users (name, email, age) VALUES (?, ?, ?)")
            .bind(&edge.name)
            .bind(&edge.email)
            .bind(edge.age)
            .execute(&pool)
            .await
            .unwrap();
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
}

/// Given legacy rows written without validation (one of them broken),
/// when they are loaded and re-checked with `validate_row` / `validate_rows`,
/// then good rows pass, the broken one reports its field, and the batch check
/// returns the index of the first broken row; an empty batch passes.
#[tokio::test]
async fn loaded_rows_are_rechecked() {
    let pool = users_db().await;
    sqlx::query(
        "INSERT INTO users (name, email, age) VALUES
            ('Test', 'test@x.com', 20), ('Nope', 'nope', 20), ('', 'bad', -1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let rows: Vec<NewUser> = sqlx::query("SELECT name, email, age FROM users ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap()
        .iter()
        .map(|r| user(r.get("name"), r.get("email"), r.get("age")))
        .collect();

    validate_row::<UserSchema, _>(&rows[0]).unwrap();
    assert_eq!(
        issues(validate_row::<UserSchema, _>(&rows[1])),
        pairs(&[(".email", "invalid_string")])
    );

    validate_rows::<UserSchema, _>(&rows[..1]).unwrap();
    validate_rows::<UserSchema, NewUser>(&[]).unwrap();
    let (index, err) = validate_rows::<UserSchema, _>(&rows).unwrap_err();
    assert_eq!(index, 1);
    assert_eq!(issues_of(&err), pairs(&[(".email", "invalid_string")]));
    let (index, err) = validate_rows::<UserSchema, _>(&rows[2..]).unwrap_err();
    assert_eq!(index, 0);
    assert_eq!(issues_of(&err).len(), 3);
}

/// A value whose `Serialize` impl fails.
struct Unserializable;
impl serde::Serialize for Unserializable {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("nope"))
    }
}

/// Given a value that cannot be serialized and a validation failure,
/// when they are turned into errors and handed to SQLx (`?` in a query
/// function),
/// then the first is `Serialization`, a `VldError` converts into `Validation`,
/// and both become `sqlx::Error::Encode` (a write-side error, not a protocol
/// error) whose text and source carry the cause.
#[test]
fn errors_become_sqlx_encode_errors() {
    use std::error::Error as _;

    let err = validate_insert::<UserSchema, _>(&Unserializable).unwrap_err();
    assert!(
        matches!(&err, VldSqlxError::Serialization(m) if m == "nope"),
        "{err:?}"
    );
    assert_eq!(err.to_string(), "Serialization error: nope");
    assert!(err.source().is_none());
    let as_sqlx: sqlx::Error = err.into();
    assert!(matches!(as_sqlx, sqlx::Error::Encode(_)), "{as_sqlx:?}");
    assert!(as_sqlx.to_string().contains("nope"));

    let vld_err = vld::error::VldError::single(vld::error::IssueCode::MissingField, "required");
    let err: VldSqlxError = vld_err.into();
    assert_eq!(issues_of(&err), pairs(&[("", "missing_field")]));
    assert!(err.to_string().starts_with("Validation error: "));
    assert!(err.source().is_some());

    let invalid = VldText::<EmailField>::new("bad").unwrap_err();
    let as_sqlx: sqlx::Error = invalid.into();
    match as_sqlx {
        sqlx::Error::Encode(inner) => {
            let back = inner
                .downcast_ref::<VldSqlxError>()
                .expect("a VldSqlxError");
            assert_eq!(issues_of(back), pairs(&[(".value", "invalid_string")]));
        }
        other => panic!("expected sqlx::Error::Encode, got {other:?}"),
    }
}
