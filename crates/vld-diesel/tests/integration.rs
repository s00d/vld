// diesel's QueryableByName derive expands to `Self { id: id, .. }`, which clippy
// >= 1.99 flags as redundant_field_names; an item attribute does not reach the impl.
#![allow(clippy::redundant_field_names)]

//! Integration tests for vld-diesel.
//! Uses an in-memory SQLite database.

use diesel::prelude::*;
use vld_diesel::prelude::*;

// ---------------------------------------------------------------------------
// Schema definitions (vld)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Diesel table + models
// ---------------------------------------------------------------------------

diesel::table! {
    users (id) {
        id -> Integer,
        name -> Text,
        email -> Text,
        age -> BigInt,
    }
}

#[derive(Debug, Insertable, serde::Serialize)]
#[diesel(table_name = users)]
struct NewUser {
    name: String,
    email: String,
    age: i64,
}

#[derive(Debug, Queryable, Selectable, serde::Serialize)]
#[diesel(table_name = users)]
struct User {
    id: i32,
    name: String,
    email: String,
    age: i64,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn setup_db() -> SqliteConnection {
    let mut conn =
        SqliteConnection::establish(":memory:").expect("Failed to create in-memory SQLite DB");
    diesel::sql_query(
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            age BIGINT NOT NULL
        )",
    )
    .execute(&mut conn)
    .expect("Failed to create table");
    conn
}

// ---------------------------------------------------------------------------
// Tests: validate_insert
// ---------------------------------------------------------------------------

/// `(path, code)` of every issue of a `Validation` error (panics on `Ok`).
#[track_caller]
fn issues<T: std::fmt::Debug>(r: Result<T, VldDieselError>) -> Vec<(String, String)> {
    match r {
        Err(VldDieselError::Validation(e)) => e
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
        VldDieselError::Validation(e) => {
            assert!(!e.issues.is_empty());
        }
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
    if let VldDieselError::Validation(e) = err {
        assert!(e.issues.len() >= 3);
    }
}

// ---------------------------------------------------------------------------
// Tests: validate_row
// ---------------------------------------------------------------------------

#[test]
fn validate_row_works() {
    let user = NewUser {
        name: "FromDB".into(),
        email: "db@example.com".into(),
        age: 50,
    };
    vld_diesel::validate_row::<UserSchema, _>(&user).unwrap();
    let bad = NewUser {
        email: "nope".into(),
        ..user
    };
    assert_eq!(
        issues(vld_diesel::validate_row::<UserSchema, _>(&bad)),
        pairs(&[(".email", "invalid_string")])
    );
}

// ---------------------------------------------------------------------------
// Tests: Validated<S, T> wrapper
// ---------------------------------------------------------------------------

#[test]
fn validated_wrapper_valid() {
    let user = NewUser {
        name: "Alice".into(),
        email: "alice@example.com".into(),
        age: 30,
    };
    let validated = Validated::<UserSchema, _>::new(user).unwrap();
    assert_eq!(validated.inner().name, "Alice");
    assert_eq!(validated.inner().email, "alice@example.com");
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
    // Deref to NewUser
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

// ---------------------------------------------------------------------------
// Tests: Validated + actual Diesel insert
// ---------------------------------------------------------------------------

#[test]
fn validated_insert_into_db() {
    let mut conn = setup_db();

    let user = NewUser {
        name: "Alice".into(),
        email: "alice@example.com".into(),
        age: 30,
    };
    let validated = Validated::<UserSchema, _>::new(user).unwrap();

    diesel::insert_into(users::table)
        .values(validated.inner())
        .execute(&mut conn)
        .expect("Insert failed");

    let loaded: Vec<User> = users::table
        .select(User::as_select())
        .load(&mut conn)
        .expect("Load failed");

    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].name, "Alice");
    assert_eq!(loaded[0].email, "alice@example.com");
    assert_eq!(loaded[0].age, 30);
}

#[test]
fn validate_then_insert() {
    let mut conn = setup_db();

    let user = NewUser {
        name: "Bob".into(),
        email: "bob@example.com".into(),
        age: 25,
    };

    validate_insert::<UserSchema, _>(&user).unwrap();

    diesel::insert_into(users::table)
        .values(&user)
        .execute(&mut conn)
        .expect("Insert failed");

    let count: i64 = users::table.count().get_result(&mut conn).unwrap();
    assert_eq!(count, 1);
}

#[test]
fn validate_row_from_db() {
    let mut conn = setup_db();

    // Insert directly without validation (simulating legacy data)
    diesel::sql_query("INSERT INTO users (name, email, age) VALUES ('Test', 'test@x.com', 20)")
        .execute(&mut conn)
        .unwrap();

    let loaded: Vec<User> = users::table
        .select(User::as_select())
        .load(&mut conn)
        .unwrap();

    for user in &loaded {
        vld_diesel::validate_row::<UserSchema, _>(user).unwrap();
    }
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].name, "Test");
}

// ---------------------------------------------------------------------------
// Tests: VldText
// ---------------------------------------------------------------------------

#[test]
fn vld_text_valid() {
    let email = VldText::<EmailFieldSchema>::new("user@example.com").unwrap();
    assert_eq!(email.as_str(), "user@example.com");
    assert_eq!(&*email, "user@example.com"); // Deref
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

// ---------------------------------------------------------------------------
// Tests: VldInt
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Tests: VldText + VldInt in Diesel models
// ---------------------------------------------------------------------------

diesel::table! {
    typed_users (id) {
        id -> Integer,
        email -> Text,
        age -> BigInt,
    }
}

#[derive(Debug, QueryableByName)]
#[diesel(table_name = typed_users)]
struct TypedUser {
    #[allow(dead_code)]
    id: i32,
    #[diesel(sql_type = diesel::sql_types::Text)]
    email: VldText<EmailFieldSchema>,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    age: VldInt<AgeFieldSchema>,
}

#[test]
fn vld_types_in_diesel_query() {
    let mut conn =
        SqliteConnection::establish(":memory:").expect("Failed to create in-memory SQLite DB");

    diesel::sql_query(
        "CREATE TABLE typed_users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            email TEXT NOT NULL,
            age BIGINT NOT NULL
        )",
    )
    .execute(&mut conn)
    .unwrap();

    diesel::sql_query("INSERT INTO typed_users (email, age) VALUES ('alice@example.com', 30)")
        .execute(&mut conn)
        .unwrap();

    let loaded: Vec<TypedUser> = diesel::sql_query("SELECT id, email, age FROM typed_users")
        .load(&mut conn)
        .unwrap();

    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].email.as_str(), "alice@example.com");
    assert_eq!(loaded[0].age.get(), 30);
}

// ---------------------------------------------------------------------------
// Tests: Error display
// ---------------------------------------------------------------------------

#[test]
fn error_display() {
    let err = VldDieselError::Serialization("oops".into());
    assert!(format!("{}", err).contains("oops"));

    let vld_err = vld::error::VldError::single(vld::error::IssueCode::MissingField, "required");
    let err = VldDieselError::Validation(vld_err);
    assert!(format!("{}", err).contains("required"));
}

#[test]
fn error_from_vld() {
    let vld_err = vld::error::VldError::single(vld::error::IssueCode::MissingField, "test");
    let err: VldDieselError = vld_err.into();
    assert!(matches!(err, VldDieselError::Validation(_)));
}

// ---------------------------------------------------------------------------
// Tests: wrappers in Insertable / Queryable / filter (AsExpression + FromSqlRow)
// ---------------------------------------------------------------------------

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

diesel::table! {
    products (id) {
        id -> Integer,
        email -> Text,
        qty -> BigInt,
        price -> Double,
        active -> Bool,
    }
}

#[derive(Insertable)]
#[diesel(table_name = products)]
struct NewProduct {
    email: VldText<EmailFieldSchema>,
    qty: VldInt<AgeFieldSchema>,
    price: VldFloat<PriceFieldSchema>,
    active: VldBool<ActiveFieldSchema>,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = products)]
struct Product {
    #[allow(dead_code)]
    id: i32,
    email: VldText<EmailFieldSchema>,
    qty: VldInt<AgeFieldSchema>,
    price: VldFloat<PriceFieldSchema>,
    active: VldBool<ActiveFieldSchema>,
}

#[test]
fn wrappers_in_insertable_queryable_and_filter() {
    let mut conn = SqliteConnection::establish(":memory:").unwrap();
    diesel::sql_query(
        "CREATE TABLE products (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            email TEXT NOT NULL,
            qty BIGINT NOT NULL,
            price DOUBLE NOT NULL,
            active BOOLEAN NOT NULL
        )",
    )
    .execute(&mut conn)
    .unwrap();

    let new = NewProduct {
        email: VldText::new("a@example.com").unwrap(),
        qty: VldInt::new(3).unwrap(),
        price: VldFloat::new(9.5).unwrap(),
        active: VldBool::new(true).unwrap(),
    };
    diesel::insert_into(products::table)
        .values(&new)
        .execute(&mut conn)
        .unwrap();

    let email = VldText::<EmailFieldSchema>::new("a@example.com").unwrap();
    let found: Vec<Product> = products::table
        .filter(products::email.eq(&email))
        .filter(products::qty.eq(VldInt::<AgeFieldSchema>::new(3).unwrap()))
        .select(Product::as_select())
        .load(&mut conn)
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].email.as_str(), "a@example.com");
    assert_eq!(found[0].qty.get(), 3);
    assert_eq!(found[0].price.get(), 9.5);
    assert!(found[0].active.get());
}

// ---------------------------------------------------------------------------
// 0.5.0 re-audit: INTEGER / REAL columns, NaN
// ---------------------------------------------------------------------------

vld::schema! {
    #[derive(Debug)]
    pub struct AnyIntSchema {
        pub value: i64 => vld::number().int(),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct OptFloatSchema {
        pub value: Option<f64> => vld::number().optional(),
    }
}

diesel::table! {
    narrow (id) {
        id -> Integer,
        n -> Integer,
        r -> Float,
    }
}

#[derive(Insertable)]
#[diesel(table_name = narrow)]
struct NewNarrow {
    n: VldInt<AnyIntSchema>,
    r: VldFloat<PriceFieldSchema>,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = narrow)]
struct Narrow {
    #[allow(dead_code)]
    id: i32,
    n: VldInt<AnyIntSchema>,
    r: VldFloat<PriceFieldSchema>,
}

#[test]
fn int_and_float_wrappers_on_integer_and_real_columns() {
    let mut conn = SqliteConnection::establish(":memory:").unwrap();
    diesel::sql_query(
        "CREATE TABLE narrow (id INTEGER PRIMARY KEY AUTOINCREMENT, n INTEGER NOT NULL, r REAL NOT NULL)",
    )
    .execute(&mut conn)
    .unwrap();

    let row = NewNarrow {
        n: VldInt::new(7).unwrap(),
        r: VldFloat::new(0.1).unwrap(),
    };
    diesel::insert_into(narrow::table)
        .values(&row)
        .execute(&mut conn)
        .unwrap();
    let found: Vec<Narrow> = narrow::table
        .filter(narrow::n.eq(VldInt::<AnyIntSchema>::new(7).unwrap()))
        .select(Narrow::as_select())
        .load(&mut conn)
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].n.get(), 7);
    assert_eq!(found[0].r.get(), 0.1);

    // Out of i32 range: an error, not a truncated value.
    let too_big = NewNarrow {
        n: VldInt::new(i64::from(i32::MAX) + 1).unwrap(),
        r: VldFloat::new(1.0).unwrap(),
    };
    assert!(
        diesel::insert_into(narrow::table)
            .values(&too_big)
            .execute(&mut conn)
            .is_err()
    );
}

#[test]
fn nan_is_rejected_explicitly() {
    // NaN serializes as JSON null; it must not pass as "missing".
    let err = VldFloat::<OptFloatSchema>::new(f64::NAN).unwrap_err();
    assert!(err.to_string().to_lowercase().contains("finite"), "{err}");
    assert!(VldFloat::<OptFloatSchema>::new(f64::INFINITY).is_err());
    assert!(VldFloat::<OptFloatSchema>::new(1.5).is_ok());
}
