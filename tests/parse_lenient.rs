use serde_json::json;
use vld::prelude::*;

vld::schema! {
    #[derive(Debug, serde::Serialize, Default, PartialEq)]
    struct Inner {
        city: String => vld::string().min(1),
    }
}

vld::schema! {
    #[derive(Debug, serde::Serialize)]
    struct TestUser {
        name: String        => vld::string().min(2).max(50),
        email: String       => vld::string().email(),
        age: Option<i64>    => vld::number().int().gte(18).optional(),
        role: String        => vld::enumeration(&["admin", "user"]).with_default("user".to_string()),
        nick: String        => vld::string().min(3).catch("anon".to_string()),
        inner: Inner        => vld::nested(Inner::parse_value),
    }
}

vld::impl_validate_fields!(TestUser {
    name  : String      => vld::string().min(2).max(50),
    email : String      => vld::string().email(),
    age   : Option<i64> => vld::number().int().gte(18).optional(),
    role  : String      => vld::enumeration(&["admin", "user"]).with_default("user".to_string()),
    nick  : String      => vld::string().min(3).catch("anon".to_string()),
    inner : Inner       => vld::nested(Inner::parse_value),
});

// ---- validate_fields ----

#[test]
fn validate_fields_all_valid() {
    let input = json!({
        "name": "Alex",
        "email": "alex@example.com",
        "age": 25,
        "role": "admin",
        "nick": "alexdev",
        "inner": { "city": "London" }
    });
    let results = TestUser::validate_fields(&input).unwrap();
    assert_eq!(results.len(), 6);
    assert!(results.iter().all(|f| f.is_ok()));
}

#[test]
fn validate_fields_mixed() {
    let input = json!({
        "name": "X",
        "email": "bad",
        "age": 25,
        "role": "admin",
        "nick": "ok_nick",
        "inner": { "city": "NY" }
    });
    let results = TestUser::validate_fields(&input).unwrap();
    let ok_count = results.iter().filter(|f| f.is_ok()).count();
    let err_count = results.iter().filter(|f| f.is_err()).count();
    assert_eq!(ok_count, 4); // age, role, nick, inner
    assert_eq!(err_count, 2); // name, email
}

#[test]
fn validate_fields_not_object() {
    let input = json!("not an object");
    let err = TestUser::validate_fields(&input).unwrap_err();
    assert!(!err.issues.is_empty());
}

// ---- parse_lenient ----

#[test]
fn parse_lenient_all_valid() {
    let input = json!({
        "name": "Alex",
        "email": "a@b.com",
        "age": 20,
        "role": "admin",
        "nick": "alexdev",
        "inner": { "city": "NY" }
    });
    let result = TestUser::parse_lenient(&input).unwrap();
    assert!(result.is_valid());
    assert!(!result.has_errors());
    assert_eq!(result.valid_count(), 6);
    assert_eq!(result.error_count(), 0);
    assert_eq!(result.value.name, "Alex");
}

#[test]
fn parse_lenient_with_errors_uses_defaults() {
    let input = json!({
        "name": "X",
        "email": "bad",
        "age": 25,
        "role": "admin",
        "nick": "!",
        "inner": { "city": "NY" }
    });
    let result = TestUser::parse_lenient(&input).unwrap();
    assert!(result.has_errors());
    assert_eq!(result.error_count(), 2); // name, email

    // Invalid fields get Default
    assert_eq!(result.value.name, ""); // String::default()
    assert_eq!(result.value.email, ""); // String::default()

    // Valid fields keep their values
    assert_eq!(result.value.age, Some(25));
    assert_eq!(result.value.role, "admin");

    // Catch still works
    assert_eq!(result.value.nick, "anon");

    // Nested valid
    assert_eq!(result.value.inner.city, "NY");
}

#[test]
fn parse_lenient_catch_field_shows_as_valid() {
    let input = json!({
        "name": "Alex",
        "email": "a@b.com",
        "age": null,
        "role": null,
        "nick": "!",
        "inner": { "city": "X" }
    });
    let result = TestUser::parse_lenient(&input).unwrap();
    // nick with catch("anon") should succeed even though "!" is too short
    let nick_field = result.fields().iter().find(|f| f.name == "nick").unwrap();
    assert!(nick_field.is_ok());
    assert_eq!(result.value.nick, "anon");
}

#[test]
fn parse_lenient_not_object() {
    let input = json!(42);
    assert!(TestUser::parse_lenient(&input).is_err());
}

// ---- ParseResult methods ----

#[test]
fn parse_result_valid_and_error_fields() {
    let input = json!({
        "name": "X",
        "email": "a@b.com",
        "age": null,
        "role": null,
        "nick": "hello",
        "inner": { "city": "A" }
    });
    let result = TestUser::parse_lenient(&input).unwrap();
    let valid = result.valid_fields();
    let errors = result.error_fields();

    assert_eq!(valid.len() + errors.len(), 6);
    assert!(errors.iter().any(|f| f.name == "name"));
    assert!(valid.iter().any(|f| f.name == "email"));
}

#[cfg(feature = "serialize")]
#[test]
fn parse_result_to_json_string() {
    let input = json!({
        "name": "Alex",
        "email": "a@b.com",
        "age": 20,
        "role": "admin",
        "nick": "dev",
        "inner": { "city": "X" }
    });
    let result = TestUser::parse_lenient(&input).unwrap();
    let json_str = result.to_json_string().unwrap();
    assert!(json_str.contains("Alex"));
    assert!(json_str.contains("a@b.com"));
}

#[cfg(feature = "serialize")]
#[test]
fn parse_result_to_json_value() {
    let input = json!({
        "name": "Alex",
        "email": "a@b.com",
        "age": null,
        "role": null,
        "nick": "dev",
        "inner": { "city": "X" }
    });
    let result = TestUser::parse_lenient(&input).unwrap();
    let val = result.to_json_value().unwrap();
    assert_eq!(val["name"], "Alex");
}

#[cfg(feature = "serialize")]
#[test]
fn parse_result_save_to_file() {
    let input = json!({
        "name": "Alex",
        "email": "a@b.com",
        "age": 20,
        "role": "admin",
        "nick": "dev",
        "inner": { "city": "X" }
    });
    let result = TestUser::parse_lenient(&input).unwrap();

    let dir = std::env::temp_dir().join("vld_test");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("test_save.json");

    result.save_to_file(&path).unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("Alex"));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn parse_result_into_value() {
    let input = json!({
        "name": "Alex",
        "email": "a@b.com",
        "age": null,
        "role": null,
        "nick": "dev",
        "inner": { "city": "X" }
    });
    let result = TestUser::parse_lenient(&input).unwrap();
    let user = result.into_value();
    assert_eq!(user.name, "Alex");
}

#[test]
fn parse_result_into_parts() {
    let input = json!({
        "name": "X",
        "email": "a@b.com",
        "age": null,
        "role": null,
        "nick": "dev",
        "inner": { "city": "X" }
    });
    let result = TestUser::parse_lenient(&input).unwrap();
    let (user, fields) = result.into_parts();
    assert_eq!(user.name, ""); // default
    assert_eq!(fields.len(), 6);
}

#[test]
fn parse_result_display() {
    let input = json!({
        "name": "X",
        "email": "a@b.com",
        "age": null,
        "role": null,
        "nick": "dev",
        "inner": { "city": "X" }
    });
    let result = TestUser::parse_lenient(&input).unwrap();
    let display = format!("{}", result);
    assert!(display.contains("valid"));
    assert!(display.contains("error"));
}

// ---- File input ----

#[cfg(feature = "std")]
#[test]
fn parse_from_file_path() {
    let dir = std::env::temp_dir().join("vld_test_input");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("input.json");
    std::fs::write(&path, r#"{"name":"Alex","email":"a@b.com","age":20,"role":"admin","nick":"dev","inner":{"city":"X"}}"#).unwrap();

    let user = TestUser::parse(path.as_path()).unwrap();
    assert_eq!(user.name, "Alex");

    let _ = std::fs::remove_file(&path);
}

#[cfg(feature = "std")]
#[test]
fn parse_lenient_from_file() {
    let dir = std::env::temp_dir().join("vld_test_input2");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("bad_input.json");
    std::fs::write(
        &path,
        r#"{"name":"X","email":"bad","age":25,"role":"admin","nick":"!","inner":{"city":"Y"}}"#,
    )
    .unwrap();

    let result = TestUser::parse_lenient(path.as_path()).unwrap();
    assert!(result.has_errors());
    assert_eq!(result.value.name, "");
    assert_eq!(result.value.nick, "anon");

    let _ = std::fs::remove_file(&path);
}

#[cfg(feature = "std")]
#[test]
fn parse_missing_file() {
    let path = std::path::Path::new("/tmp/vld_nonexistent_file.json");
    let err = TestUser::parse(path).unwrap_err();
    assert!(!err.issues.is_empty());
}
