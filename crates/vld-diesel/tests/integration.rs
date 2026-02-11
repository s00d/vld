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

#[test]
fn validate_insert_valid() {
    let user = NewUser {
        name: "Alice".into(),
        email: "alice@example.com".into(),
        age: 30,
    };
    assert!(validate_insert::<UserSchema, _>(&user).is_ok());
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
    assert!(validate_insert::<UserSchema, _>(&user).is_err());
}

#[test]
fn validate_insert_empty_name() {
    let user = NewUser {
        name: "".into(),
        email: "x@y.com".into(),
        age: 20,
    };
    assert!(validate_insert::<UserSchema, _>(&user).is_err());
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
// Tests: validate_update / validate_row (same logic)
// ---------------------------------------------------------------------------

#[test]
fn validate_update_works() {
    let user = NewUser {
        name: "Updated".into(),
        email: "updated@example.com".into(),
        age: 40,
    };
    assert!(vld_diesel::validate_update::<UserSchema, _>(&user).is_ok());
}

#[test]
fn validate_row_works() {
    let user = NewUser {
        name: "FromDB".into(),
        email: "db@example.com".into(),
        age: 50,
    };
    assert!(vld_diesel::validate_row::<UserSchema, _>(&user).is_ok());
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
    assert!(Validated::<UserSchema, _>::new(user).is_err());
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
        assert!(vld_diesel::validate_row::<UserSchema, _>(user).is_ok());
    }
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
    assert!(VldText::<EmailFieldSchema>::new("not-an-email").is_err());
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
    let result: Result<VldText<EmailFieldSchema>, _> =
        serde_json::from_value(serde_json::json!("bad"));
    assert!(result.is_err());
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
    assert!(VldInt::<AgeFieldSchema>::new(-1).is_err());
    assert!(VldInt::<AgeFieldSchema>::new(200).is_err());
}

#[test]
fn vld_int_unchecked() {
    let v = VldInt::<AgeFieldSchema>::new_unchecked(999);
    assert_eq!(*v, 999);
}

#[test]
fn vld_int_serialize() {
    let v = VldInt::<AgeFieldSchema>::new(42).unwrap();
    let json = serde_json::to_value(&v).unwrap();
    assert_eq!(json, serde_json::json!(42));
}

#[test]
fn vld_int_deserialize() {
    let v: VldInt<AgeFieldSchema> = serde_json::from_value(serde_json::json!(30)).unwrap();
    assert_eq!(*v, 30);
}

#[test]
fn vld_int_deserialize_invalid() {
    let result: Result<VldInt<AgeFieldSchema>, _> = serde_json::from_value(serde_json::json!(-5));
    assert!(result.is_err());
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
