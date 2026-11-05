//! `optional`, `nullable`, `nullish`, `with_default`, `catch`, `describe`, `message`.

use crate::common::{codes, parse_err};
use serde_json::json;
use vld::prelude::*;

#[test]
fn optional_nullable_nullish_map_null_to_none() {
    let o = vld::string().optional();
    let n = vld::string().nullable();
    let nn = vld::string().nullish();
    assert_eq!(o.parse_value(&json!(null)).unwrap(), None);
    assert_eq!(n.parse_value(&json!(null)).unwrap(), None);
    assert_eq!(nn.parse_value(&json!(null)).unwrap(), None);
    assert_eq!(o.parse_value(&json!("hi")).unwrap(), Some("hi".to_string()));
    assert_eq!(
        nn.parse_value(&json!("hi")).unwrap(),
        Some("hi".to_string())
    );
    // A present value is still validated.
    assert_eq!(codes(&parse_err(&o, json!(1))), ["invalid_type"]);
    assert_eq!(
        codes(&parse_err(&vld::string().min(3).nullable(), json!("a"))),
        ["too_small"]
    );
}

#[test]
fn with_default_only_replaces_null() {
    let s = vld::string().with_default("world".to_string());
    assert_eq!(s.parse_value(&json!(null)).unwrap(), "world");
    assert_eq!(s.parse_value(&json!("hello")).unwrap(), "hello");
    assert_eq!(s.parse_value(&json!("")).unwrap(), "");
    assert_eq!(codes(&parse_err(&s, json!(42))), ["invalid_type"]);
}

#[test]
fn catch_replaces_any_failure() {
    let s = vld::string().min(6).catch("fallback".to_string());
    assert_eq!(s.parse_value(&json!("short")).unwrap(), "fallback");
    assert_eq!(s.parse_value(&json!(42)).unwrap(), "fallback");
    assert_eq!(s.parse_value(&json!(null)).unwrap(), "fallback");
    assert_eq!(s.parse_value(&json!("long enough")).unwrap(), "long enough");
}

#[test]
fn describe_keeps_validation_and_text() {
    let s = vld::string().min(3).describe("User's full name");
    assert_eq!(s.description(), "User's full name");
    assert!(s.parse(r#""Alex""#).is_ok());
    assert_eq!(codes(&s.parse(r#""Al""#).unwrap_err()), ["too_small"]);
}

#[test]
fn message_replaces_every_issue_message_and_keeps_codes() {
    let s = vld::number().int().min(0).max(100).message("Bad number");
    let err = parse_err(&s, json!("not a number"));
    assert!(err.issues.iter().all(|i| i.message == "Bad number"));
    assert_eq!(codes(&err), ["invalid_type"]);
    let err = parse_err(&vld::string().email().message("Invalid email"), json!("x"));
    assert_eq!(err.issues[0].message, "Invalid email");
    assert_eq!(codes(&err), ["invalid_string"]);
    assert_eq!(
        vld::string()
            .min(2)
            .message("e")
            .parse(r#""hello""#)
            .unwrap(),
        "hello"
    );
}

#[test]
fn default_value_accessor() {
    let s = vld::number().int().with_default(7);
    assert_eq!(*s.default_value(), 7);
    assert_eq!(s.inner_schema().parse_value(&json!(1)).unwrap(), 1);
}
