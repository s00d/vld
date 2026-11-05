//! `impl_validate_fields!` (validate_fields / parse_lenient / ParseResult)
//! and `impl_rules!`.

use crate::common::{paths, tempdir};
use serde_json::json;

vld::schema! {
    #[derive(Debug, serde::Serialize, Default, PartialEq)]
    struct Inner {
        city: String => vld::string().min(1),
    }
}

vld::schema! {
    #[derive(Debug, serde::Serialize)]
    struct Profile {
        name: String        => vld::string().min(2).max(50),
        email: String       => vld::string().email(),
        age: Option<i64>    => vld::number().int().gte(18).optional(),
        role: String        => vld::enumeration(&["admin", "user"]).with_default("user".to_string()),
        nick: String        => vld::string().min(3).catch("anon".to_string()),
        inner: Inner        => vld::nested(Inner::parse_value),
    }
}

vld::impl_validate_fields!(Profile {
    name  : String      => vld::string().min(2).max(50),
    email : String      => vld::string().email(),
    age   : Option<i64> => vld::number().int().gte(18).optional(),
    role  : String      => vld::enumeration(&["admin", "user"]).with_default("user".to_string()),
    nick  : String      => vld::string().min(3).catch("anon".to_string()),
    inner : Inner       => vld::nested(Inner::parse_value),
});

fn input(name: &str, email: &str, nick: &str) -> serde_json::Value {
    json!({"name": name, "email": email, "age": 25, "role": "admin", "nick": nick, "inner": {"city": "NY"}})
}

#[test]
fn validate_fields_reports_per_field() {
    let fields = Profile::validate_fields(&input("X", "bad", "ok_nick")).unwrap();
    let state: Vec<(&str, bool)> = fields
        .iter()
        .map(|f| (f.name.as_str(), f.is_ok()))
        .collect();
    assert_eq!(
        state,
        [
            ("name", false),
            ("email", false),
            ("age", true),
            ("role", true),
            ("nick", true),
            ("inner", true)
        ]
    );
    assert!(fields[1].is_err());
    let err = Profile::validate_fields(&json!("not an object")).unwrap_err();
    assert_eq!(err.issues[0].code.key(), "invalid_type");
}

#[test]
fn parse_lenient_defaults_invalid_fields() {
    let r = Profile::parse_lenient(&input("Alex", "a@b.com", "alexdev")).unwrap();
    assert!(r.is_valid() && !r.has_errors());
    assert_eq!((r.valid_count(), r.error_count()), (6, 0));

    let r = Profile::parse_lenient(&input("X", "bad", "!")).unwrap();
    assert_eq!(r.error_count(), 2);
    assert_eq!((r.value.name.as_str(), r.value.email.as_str()), ("", ""));
    assert_eq!(r.value.age, Some(25));
    assert_eq!(r.value.role, "admin");
    // `catch` turns the failure into a valid fallback.
    assert_eq!(r.value.nick, "anon");
    assert!(r.field("nick").unwrap().is_ok());
    assert_eq!(r.value.inner.city, "NY");
    let names: Vec<&str> = r.error_fields().iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["name", "email"]);
    assert_eq!(r.valid_fields().len(), 4);
    assert!(Profile::parse_lenient(&json!(42)).is_err());
}

#[test]
fn parse_result_outputs() {
    let r = Profile::parse_lenient(&input("X", "a@b.com", "dev")).unwrap();
    let text = r.to_string();
    assert!(text.contains("valid") && text.contains("error"), "{text}");
    assert_eq!(r.to_json_value().unwrap()["name"], "");
    assert!(r.to_json_string().unwrap().contains("a@b.com"));

    let dir = tempdir();
    let path = dir.path().join("out.json");
    r.save_to_file(&path).unwrap();
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved["email"], "a@b.com");

    let (user, fields) = r.into_parts();
    assert_eq!((user.name.as_str(), fields.len()), ("", 6));
    let r = Profile::parse_lenient(&input("Alex", "a@b.com", "dev")).unwrap();
    assert_eq!(r.into_value().name, "Alex");
}

#[test]
fn parse_from_files() {
    let dir = tempdir();
    let good = dir.path().join("good.json");
    std::fs::write(&good, input("Alex", "a@b.com", "dev").to_string()).unwrap();
    assert_eq!(Profile::parse(good.as_path()).unwrap().name, "Alex");

    let bad = dir.path().join("bad.json");
    std::fs::write(&bad, input("X", "bad", "!").to_string()).unwrap();
    let r = Profile::parse_lenient(bad.as_path()).unwrap();
    assert_eq!((r.value.name.as_str(), r.value.nick.as_str()), ("", "anon"));

    let err = Profile::parse(dir.path().join("missing.json").as_path()).unwrap_err();
    assert_eq!(err.issues[0].code.key(), "io_error");
}

// --- renames in impl_validate_fields! ---------------------------------------

vld::schema! {
    #[derive(Debug, Default, serde::Serialize)]
    pub struct RenamedProfile {
        pub user_name: String as "userName" => vld::string().min(2),
        pub age: Option<i64> => vld::number().int().min(0).optional(),
    }
}

vld::impl_validate_fields!(RenamedProfile {
    user_name: String as "userName" => vld::string().min(2),
    age: Option<i64> => vld::number().int().min(0).optional(),
});

#[test]
fn validate_fields_and_lenient_use_renamed_keys() {
    let fields = RenamedProfile::validate_fields(r#"{"userName": "Al", "age": 25}"#).unwrap();
    assert_eq!(fields[0].name, "userName");
    assert!(fields[0].is_ok());
    let r = RenamedProfile::parse_lenient(r#"{"userName": "X", "age": 25}"#).unwrap();
    assert!(r.field("userName").unwrap().is_err());
    assert!(r.field("user_name").is_none());
}

// --- impl_rules! ---------------------------------------------------------------

struct Wire {
    first_name: String,
    count: i64,
}

vld::impl_rules!(Wire {
    first_name as "firstName" => vld::string().min(2),
    count => vld::number().int().min(1),
});

#[test]
fn impl_rules_validates_fields_with_renames() {
    let ok = Wire {
        first_name: "Al".into(),
        count: 1,
    };
    assert!(ok.validate().is_ok());
    assert!(ok.is_valid());
    let bad = Wire {
        first_name: "x".into(),
        count: 0,
    };
    assert_eq!(
        paths(&bad.validate().unwrap_err()),
        [".firstName", ".count"]
    );
    assert!(!bad.is_valid());
}
