//! Regression tests for the live end-to-end findings (0.5.0).

use serde_json::json;
use vld::error::{IssueCode, VldError};
use vld::prelude::*;

// ---- parse_coerced: values coerced from text follow the schema ----------

vld::schema! {
    #[derive(Debug)]
    struct Query {
        q: String => vld::string(),
        age: i64 => vld::number().int(),
        zip: String => vld::string(),
        tags: Vec<String> => vld::array(vld::string()),
        ids: Vec<String> => vld::array(vld::string()),
        flag: String => vld::string(),
    }
}

#[test]
fn parse_coerced_follows_schema() {
    let v = json!({"q": 123, "age": 30, "zip": "01234", "tags": "a", "ids": 7, "flag": true});
    let q: Query = vld::schema::parse_coerced(&v).unwrap();
    assert_eq!(q.q, "123");
    assert_eq!(q.age, 30);
    assert_eq!(q.zip, "01234");
    assert_eq!(q.tags, ["a"]);
    assert_eq!(q.ids, ["7"]);
    assert_eq!(q.flag, "true");
}

#[test]
fn parse_coerced_keeps_real_errors() {
    let v = json!({"q": 1, "age": "abc", "zip": "1", "tags": [], "ids": [], "flag": "x"});
    let err = vld::schema::parse_coerced::<Query>(&v).unwrap_err();
    assert_eq!(err.issues.len(), 1);
    assert!(matches!(err.issues[0].code, IssueCode::InvalidType { .. }));
}

// ---- i18n -------------------------------------------------------------------

#[test]
fn translations_cover_types_and_units() {
    let ru = vld::i18n::russian();
    let err = vld::string().parse_value(&json!(true)).unwrap_err();
    let msg = &vld::i18n::translate_error(&err, &ru).issues[0].message;
    assert_eq!(msg, "Ожидалось строка, получено логическое значение");

    let err = vld::string().min(3).parse_value(&json!("ab")).unwrap_err();
    let msg = &vld::i18n::translate_error(&err, &ru).issues[0].message;
    assert!(msg.contains("3 символов"), "{msg}");

    for lang in [
        vld::i18n::russian(),
        vld::i18n::german(),
        vld::i18n::spanish(),
    ] {
        let err = vld::literal("a").parse_value(&json!("b")).unwrap_err();
        let msg = &vld::i18n::translate_error(&err, &lang).issues[0].message;
        assert!(!msg.starts_with("Expected"), "{msg}");
    }
}

vld::schema! {
    #[derive(Debug)]
    struct Person {
        name: String => vld::string(),
        nick: Option<String> => vld::string().optional(),
    }
}

#[test]
fn absent_required_field_is_missing_field() {
    let err = Person::parse_value(&json!({})).unwrap_err();
    assert_eq!(err.issues.len(), 1);
    assert_eq!(err.issues[0].code, IssueCode::MissingField);

    // Present but null: still a type error.
    let err = Person::parse_value(&json!({"name": null})).unwrap_err();
    assert!(matches!(err.issues[0].code, IssueCode::InvalidType { .. }));

    let obj = vld::object().field("name", vld::string());
    let err = obj.parse_value(&json!({})).unwrap_err();
    assert_eq!(err.issues[0].code, IssueCode::MissingField);
}

// ---- messages -----------------------------------------------------------------

#[test]
fn display_does_not_repeat_received() {
    let err = vld::string().parse_value(&json!(null)).unwrap_err();
    assert_eq!(err.to_string(), "Expected string, received null");

    let err = vld::union(vld::string(), vld::number())
        .parse_value(&json!(true))
        .unwrap_err();
    let text = err.to_string();
    assert!(!text.contains("received boolean, received"), "{text}");
}

#[test]
fn integer_reports_the_strictest_bound() {
    let err = vld::u8().max(150).parse_value(&json!(300)).unwrap_err();
    assert!(err.issues[0].message.contains("150"), "{err}");
    let err = vld::u8().min(10).parse_value(&json!(-5)).unwrap_err();
    assert!(err.issues[0].message.contains("10"), "{err}");
    // Without a user bound the type range is reported.
    let err = vld::u8().parse_value(&json!(300)).unwrap_err();
    assert!(err.issues[0].message.contains("255"), "{err}");
}

// ---- inner failures without issues are failures ---------------------------

/// Always fails, with an error that has no issues.
#[derive(Clone, Copy)]
struct Silent0;

impl VldSchema for Silent0 {
    type Output = i64;
    fn parse_value(&self, _: &serde_json::Value) -> Result<i64, VldError> {
        Err(VldError::new())
    }
}

#[cfg(feature = "openapi")]
impl vld::json_schema::JsonSchema for Silent0 {
    fn json_schema(&self) -> serde_json::Value {
        json!({})
    }
}

#[cfg(feature = "openapi")]
impl vld::json_schema::CollectNestedSchemas for Silent0 {}

fn fail_silently() -> Silent0 {
    Silent0
}

#[test]
fn issue_less_inner_errors_fail_the_container() {
    assert!(vld::array(fail_silently())
        .parse_value(&json!([1]))
        .is_err());
    assert!(vld::set(fail_silently()).parse_value(&json!([1])).is_err());
    assert!(vld::record(fail_silently())
        .parse_value(&json!({"a": 1}))
        .is_err());
    assert!(vld::map(vld::string(), fail_silently())
        .parse_value(&json!([["a", 1]]))
        .is_err());
    assert!(vld::intersection(vld::number(), fail_silently())
        .parse_value(&json!(1))
        .is_err());
}

vld::schema! {
    struct Silent {
        x: i64 => fail_silently(),
    }
}

#[test]
fn issue_less_field_error_fails_schema_without_panic() {
    assert!(Silent::parse_value(&json!({"x": 1})).is_err());
}

// ---- bytes().base64url() --------------------------------------------------------

#[test]
fn base64url_bytes_reject_standard_alphabet() {
    let s = vld::bytes().base64url();
    assert!(s.parse_value(&json!("-_8")).is_ok());
    assert!(s.parse_value(&json!("+/8")).is_err());
    assert!(vld::string()
        .base64url()
        .parse_value(&json!("a+b"))
        .is_err());
}

// ---- time datetime default is RFC 3339 -------------------------------------------

#[cfg(all(feature = "time", feature = "openapi"))]
#[test]
fn time_datetime_default_is_rfc3339() {
    use vld::json_schema::JsonSchema;
    let dt = vld::time::datetime()
        .parse_value(&json!("2024-01-02T03:04:05Z"))
        .unwrap();
    let schema = vld::time::datetime().with_default(dt).json_schema();
    let default = schema["default"].clone();
    assert!(
        vld::time::datetime().parse_value(&default).is_ok(),
        "{default}"
    );
}
