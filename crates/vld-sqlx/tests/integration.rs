#[cfg(feature = "sqlx-0_8")]
use sqlx08 as sqlx;
#[cfg(feature = "sqlx-0_9")]
use sqlx09 as sqlx;

use sqlx::{Row, SqlitePool};
use vld_sqlx::prelude::*;

// ========================= Schema definitions ================================

vld::schema! {
    #[derive(Debug)]
    pub struct UserSchema {
        pub name: String  => vld::string().min(1).max(100),
        pub email: String => vld::string().email(),
        pub age: i64      => vld::number().int().min(0).max(150),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct EmailFieldSchema {
        pub value: String => vld::string().email(),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct AgeFieldSchema {
        pub value: i64 => vld::number().int().min(0).max(150),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct PriceFieldSchema {
        pub value: f64 => vld::number().min(0.0),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct ActiveFieldSchema {
        pub value: bool => vld::boolean(),
    }
}

// ========================= Serializable models ===============================

#[derive(Debug, serde::Serialize)]
struct NewUser {
    name: String,
    email: String,
    age: i64,
}

// ========================= Tests: validate_insert ============================

/// `(path, code)` of every issue of a `Validation` error (panics on `Ok`).
#[track_caller]
fn issues<T: std::fmt::Debug>(r: Result<T, VldSqlxError>) -> Vec<(String, String)> {
    match r {
        Err(VldSqlxError::Validation(e)) => e
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

#[test]
fn validate_insert_valid() {
    let user = NewUser {
        name: "Alice".into(),
        email: "alice@example.com".into(),
        age: 30,
    };
    validate_insert::<UserSchema, _>(&user).unwrap();
    // Bounds are inclusive.
    let edge = NewUser {
        name: "A".into(),
        email: "a@b.co".into(),
        age: 150,
    };
    validate_insert::<UserSchema, _>(&edge).unwrap();
}

#[test]
fn validate_insert_invalid_email() {
    let user = NewUser {
        name: "Bob".into(),
        email: "not-an-email".into(),
        age: 25,
    };
    let err = validate_insert::<UserSchema, _>(&user);
    assert!(err.is_err());
    match err.unwrap_err() {
        VldSqlxError::Validation(e) => assert!(!e.issues.is_empty()),
        other => panic!("Expected Validation error, got {:?}", other),
    }
}

#[test]
fn validate_insert_invalid_age() {
    let user = NewUser {
        name: "Charlie".into(),
        email: "charlie@example.com".into(),
        age: -5,
    };
    assert_eq!(
        issues(validate_insert::<UserSchema, _>(&user)),
        pairs(&[(".age", "too_small")])
    );
}

#[test]
fn validate_insert_empty_name() {
    let user = NewUser {
        name: "".into(),
        email: "x@y.com".into(),
        age: 20,
    };
    assert_eq!(
        issues(validate_insert::<UserSchema, _>(&user)),
        pairs(&[(".name", "too_small")])
    );
}

#[test]
fn validate_insert_multiple_errors() {
    let user = NewUser {
        name: "".into(),
        email: "bad".into(),
        age: -1,
    };
    let err = validate_insert::<UserSchema, _>(&user).unwrap_err();
    if let VldSqlxError::Validation(e) = err {
        assert!(e.issues.len() >= 3);
    }
}

// ========================= Tests: validate_row ===============================

#[test]
fn validate_row_works() {
    let user = NewUser {
        name: "FromDB".into(),
        email: "db@example.com".into(),
        age: 50,
    };
    validate_row::<UserSchema, _>(&user).unwrap();
    let bad = NewUser {
        email: "nope".into(),
        ..user
    };
    assert_eq!(
        issues(validate_row::<UserSchema, _>(&bad)),
        pairs(&[(".email", "invalid_string")])
    );
}

// ========================= Tests: validate_rows ==============================

#[test]
fn validate_rows_all_valid() {
    let rows = vec![
        NewUser {
            name: "A".into(),
            email: "a@b.com".into(),
            age: 10,
        },
        NewUser {
            name: "B".into(),
            email: "b@c.com".into(),
            age: 20,
        },
    ];
    validate_rows::<UserSchema, _>(&rows).unwrap();
    validate_rows::<UserSchema, NewUser>(&[]).unwrap();
}

#[test]
fn validate_rows_one_invalid() {
    let rows = vec![
        NewUser {
            name: "A".into(),
            email: "a@b.com".into(),
            age: 10,
        },
        NewUser {
            name: "".into(),
            email: "bad".into(),
            age: -1,
        },
    ];
    let err = validate_rows::<UserSchema, _>(&rows).unwrap_err();
    assert_eq!(err.0, 1); // index of bad row
}

// ========================= Tests: Validated<S, T> ============================

#[test]
fn validated_wrapper_valid() {
    let user = NewUser {
        name: "Alice".into(),
        email: "alice@example.com".into(),
        age: 30,
    };
    let validated = Validated::<UserSchema, _>::new(user).unwrap();
    assert_eq!(validated.inner().name, "Alice");
}

#[test]
fn validated_wrapper_invalid() {
    let user = NewUser {
        name: "".into(),
        email: "bad".into(),
        age: -1,
    };
    assert_eq!(
        issues(Validated::<UserSchema, _>::new(user)),
        pairs(&[
            (".name", "too_small"),
            (".email", "invalid_string"),
            (".age", "too_small")
        ])
    );
}

#[test]
fn validated_wrapper_deref() {
    let user = NewUser {
        name: "Bob".into(),
        email: "bob@test.com".into(),
        age: 25,
    };
    let validated = Validated::<UserSchema, _>::new(user).unwrap();
    assert_eq!(validated.name, "Bob");
}

#[test]
fn validated_wrapper_into_inner() {
    let user = NewUser {
        name: "Carol".into(),
        email: "carol@test.com".into(),
        age: 35,
    };
    let validated = Validated::<UserSchema, _>::new(user).unwrap();
    let inner = validated.into_inner();
    assert_eq!(inner.name, "Carol");
}

// ========================= Tests: VldText ====================================

#[test]
fn vld_text_valid() {
    let email = VldText::<EmailFieldSchema>::new("user@example.com").unwrap();
    assert_eq!(email.as_str(), "user@example.com");
    assert_eq!(&*email, "user@example.com");
    assert_eq!(format!("{}", email), "user@example.com");
}

#[test]
fn vld_text_invalid() {
    assert_eq!(
        issues(VldText::<EmailFieldSchema>::new("not-an-email")),
        pairs(&[(".value", "invalid_string")])
    );
}

#[test]
fn vld_text_unchecked() {
    let text = VldText::<EmailFieldSchema>::new_unchecked("anything");
    assert_eq!(text.as_str(), "anything");
}

#[test]
fn vld_text_serialize() {
    let email = VldText::<EmailFieldSchema>::new("a@b.com").unwrap();
    let json = serde_json::to_value(&email).unwrap();
    assert_eq!(json, serde_json::json!("a@b.com"));
}

#[test]
fn vld_text_deserialize() {
    let email: VldText<EmailFieldSchema> =
        serde_json::from_value(serde_json::json!("a@b.com")).unwrap();
    assert_eq!(email.as_str(), "a@b.com");
}

#[test]
fn vld_text_deserialize_invalid() {
    let err =
        serde_json::from_value::<VldText<EmailFieldSchema>>(serde_json::json!("bad")).unwrap_err();
    assert!(
        err.to_string().starts_with("Validation error: .value: "),
        "{err}"
    );
}

#[test]
fn vld_text_clone_eq() {
    let a = VldText::<EmailFieldSchema>::new("x@y.com").unwrap();
    let b = a.clone();
    assert_eq!(a, b);
}

// ========================= Tests: VldInt =====================================

#[test]
fn vld_int_valid() {
    let age = VldInt::<AgeFieldSchema>::new(25).unwrap();
    assert_eq!(*age, 25);
    assert_eq!(age.get(), 25);
    assert_eq!(format!("{}", age), "25");
}

#[test]
fn vld_int_invalid() {
    assert_eq!(
        issues(VldInt::<AgeFieldSchema>::new(-1)),
        pairs(&[(".value", "too_small")])
    );
    assert_eq!(
        issues(VldInt::<AgeFieldSchema>::new(200)),
        pairs(&[(".value", "too_big")])
    );
}

#[test]
fn vld_int_unchecked() {
    let v = VldInt::<AgeFieldSchema>::new_unchecked(999);
    assert_eq!(*v, 999);
}

#[test]
fn vld_int_serialize() {
    let v = VldInt::<AgeFieldSchema>::new(42).unwrap();
    let json = serde_json::to_value(v).unwrap();
    assert_eq!(json, serde_json::json!(42));
}

#[test]
fn vld_int_deserialize() {
    let v: VldInt<AgeFieldSchema> = serde_json::from_value(serde_json::json!(30)).unwrap();
    assert_eq!(*v, 30);
}

#[test]
fn vld_int_deserialize_invalid() {
    let err = serde_json::from_value::<VldInt<AgeFieldSchema>>(serde_json::json!(-5)).unwrap_err();
    assert!(
        err.to_string().starts_with("Validation error: .value: "),
        "{err}"
    );
}

// ========================= Tests: VldFloat ===================================

#[test]
fn vld_float_valid() {
    let price = VldFloat::<PriceFieldSchema>::new(9.99).unwrap();
    assert!((*price - 9.99).abs() < f64::EPSILON);
}

#[test]
fn vld_float_invalid() {
    assert_eq!(
        issues(VldFloat::<PriceFieldSchema>::new(-0.01)),
        pairs(&[(".value", "too_small")])
    );
}

#[test]
fn vld_float_serialize() {
    let expected: f64 = "3.14".parse().unwrap();
    let v = VldFloat::<PriceFieldSchema>::new(expected).unwrap();
    let json = serde_json::to_value(v).unwrap();
    assert!((json.as_f64().unwrap() - expected).abs() < f64::EPSILON);
}

// ========================= Tests: VldBool ====================================

#[test]
fn vld_bool_valid() {
    let active = VldBool::<ActiveFieldSchema>::new(true).unwrap();
    assert!(*active);
    assert!(active.get());
}

#[test]
fn vld_bool_serialize() {
    let v = VldBool::<ActiveFieldSchema>::new(false).unwrap();
    let json = serde_json::to_value(v).unwrap();
    assert_eq!(json, serde_json::json!(false));
}

// ========================= Tests: Error type =================================

#[test]
fn error_display() {
    let err = VldSqlxError::Serialization("oops".into());
    assert!(format!("{}", err).contains("oops"));

    let vld_err = vld::error::VldError::single(vld::error::IssueCode::MissingField, "required");
    let err = VldSqlxError::Validation(vld_err);
    assert!(format!("{}", err).contains("required"));
}

#[test]
fn error_from_vld() {
    let vld_err = vld::error::VldError::single(vld::error::IssueCode::MissingField, "test");
    let err: VldSqlxError = vld_err.into();
    assert!(matches!(err, VldSqlxError::Validation(_)));
}

#[test]
fn error_into_sqlx_error() {
    let err = VldSqlxError::Serialization("boom".into());
    let sqlx_err: sqlx::Error = err.into();
    assert!(sqlx_err.to_string().contains("boom"));
}

// ========================= Tests: SQLx DB integration ========================

async fn setup_pool() -> SqlitePool {
    let pool = SqlitePool::connect(":memory:").await.unwrap();
    sqlx::query(
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            age INTEGER NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool
}

#[tokio::test]
async fn validated_insert_into_db() {
    let pool = setup_pool().await;

    let user = NewUser {
        name: "Alice".into(),
        email: "alice@example.com".into(),
        age: 30,
    };
    let validated = Validated::<UserSchema, _>::new(user).unwrap();

    sqlx::query("INSERT INTO users (name, email, age) VALUES (?, ?, ?)")
        .bind(&validated.inner().name)
        .bind(&validated.inner().email)
        .bind(validated.inner().age)
        .execute(&pool)
        .await
        .unwrap();

    let row = sqlx::query("SELECT name, email, age FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();

    let name: String = row.get("name");
    let email: String = row.get("email");
    let age: i64 = row.get("age");

    assert_eq!(name, "Alice");
    assert_eq!(email, "alice@example.com");
    assert_eq!(age, 30);
}

#[tokio::test]
async fn validate_then_insert() {
    let pool = setup_pool().await;

    let user = NewUser {
        name: "Bob".into(),
        email: "bob@example.com".into(),
        age: 25,
    };
    validate_insert::<UserSchema, _>(&user).unwrap();

    sqlx::query("INSERT INTO users (name, email, age) VALUES (?, ?, ?)")
        .bind(&user.name)
        .bind(&user.email)
        .bind(user.age)
        .execute(&pool)
        .await
        .unwrap();

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn validate_row_from_db() {
    let pool = setup_pool().await;

    sqlx::query("INSERT INTO users (name, email, age) VALUES ('Test', 'test@x.com', 20)")
        .execute(&pool)
        .await
        .unwrap();

    let rows = sqlx::query("SELECT name, email, age FROM users")
        .fetch_all(&pool)
        .await
        .unwrap();

    for row in &rows {
        let user = NewUser {
            name: row.get("name"),
            email: row.get("email"),
            age: row.get("age"),
        };
        validate_row::<UserSchema, _>(&user).unwrap();
        assert_eq!((user.name.as_str(), user.age), ("Test", 20));
    }
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn vld_text_encode_decode() {
    let pool = setup_pool().await;

    let email = VldText::<EmailFieldSchema>::new("alice@test.com").unwrap();

    sqlx::query("INSERT INTO users (name, email, age) VALUES ('A', ?, 25)")
        .bind(&email)
        .execute(&pool)
        .await
        .unwrap();

    let row = sqlx::query("SELECT email FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();

    let decoded: VldText<EmailFieldSchema> = row.get("email");
    assert_eq!(decoded.as_str(), "alice@test.com");
}

#[tokio::test]
async fn vld_int_encode_decode() {
    let pool = setup_pool().await;

    let age = VldInt::<AgeFieldSchema>::new(30).unwrap();

    sqlx::query("INSERT INTO users (name, email, age) VALUES ('A', 'a@b.com', ?)")
        .bind(age)
        .execute(&pool)
        .await
        .unwrap();

    let row = sqlx::query("SELECT age FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();

    let decoded: VldInt<AgeFieldSchema> = row.get("age");
    assert_eq!(decoded.get(), 30);
}

#[tokio::test]
async fn vld_int_decodes_small_integer_columns() {
    let pool = SqlitePool::connect(":memory:").await.unwrap();
    sqlx::query("CREATE TABLE t (a INT4 NOT NULL, b SMALLINT NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO t (a, b) VALUES (?, ?)")
        .bind(VldInt::<AgeFieldSchema>::new(30).unwrap())
        .bind(VldInt::<AgeFieldSchema>::new(7).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let row = sqlx::query("SELECT a, b FROM t")
        .fetch_one(&pool)
        .await
        .unwrap();
    let a: VldInt<AgeFieldSchema> = row.try_get("a").unwrap();
    let b: VldInt<AgeFieldSchema> = row.try_get("b").unwrap();
    assert_eq!((a.get(), b.get()), (30, 7));
}

// ========================= 0.5.0 re-audit =====================================

/// A validation failure is an encode-side error, not a protocol error.
#[test]
fn validation_error_maps_to_encode() {
    let e = VldText::<EmailFieldSchema>::new("bad").unwrap_err();
    let sqlx_err: sqlx::Error = e.into();
    assert!(matches!(sqlx_err, sqlx::Error::Encode(_)), "{sqlx_err:?}");
}

#[test]
fn vld_float_rejects_nan() {
    vld::schema! {
        #[derive(Debug)]
        pub struct OptFloat {
            pub value: Option<f64> => vld::number().optional(),
        }
    }
    // NaN serializes as null, which must not satisfy `optional()`.
    assert_eq!(
        issues(VldFloat::<OptFloat>::new(f64::NAN)),
        pairs(&[("", "not_finite")])
    );
    assert_eq!(*VldFloat::<OptFloat>::new(2.5).unwrap(), 2.5);
}

#[tokio::test]
async fn vld_float_round_trips_through_real_column() {
    let pool = SqlitePool::connect(":memory:").await.unwrap();
    sqlx::query("CREATE TABLE f (r REAL NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO f (r) VALUES (?)")
        .bind(VldFloat::<PriceFieldSchema>::new(0.1).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let row = sqlx::query("SELECT r FROM f")
        .fetch_one(&pool)
        .await
        .unwrap();
    let r: VldFloat<PriceFieldSchema> = row.try_get("r").unwrap();
    assert_eq!(r.get(), 0.1);
}
