//! Optional fields: `field_optional`, `partial`, `required`, `deep_partial`.
//! Absent and `null` keys are tested separately.

use crate::common::{parse_err, paths};
use serde_json::json;
use vld::error::IssueCode;
use vld::prelude::*;

fn person() -> vld::object::ZObject {
    vld::object()
        .field("name", vld::string().min(1))
        .field("age", vld::number().int())
}

#[test]
fn partial_accepts_absent_and_null_but_validates_present() {
    let s = person().partial();
    for input in [json!({}), json!({"name": null, "age": null})] {
        let out = s.parse_value(&input).unwrap();
        assert_eq!(out["name"], json!(null), "{input}");
        assert_eq!(out["age"], json!(null), "{input}");
    }
    let err = parse_err(&s, json!({"name": "", "age": "x"}));
    assert_eq!(paths(&err), [".name", ".age"]);
}

#[test]
fn partial_keeps_inner_default_and_catch() {
    let s = vld::object()
        .field("n", vld::number().with_default(5.0))
        .field("s", vld::string().min(3).catch("anon".to_string()))
        .field("plain", vld::string())
        .partial();
    let out = s.parse_value(&json!({})).unwrap();
    assert_eq!(out["n"], json!(5.0));
    assert_eq!(out["s"], json!("anon"));
    assert_eq!(out["plain"], json!(null));
}

#[test]
fn required_undoes_partial_for_absent_and_null() {
    let s = person().partial().required();
    for input in [json!({"age": 1}), json!({"name": null, "age": 1})] {
        let err = parse_err(&s, input);
        assert_eq!(paths(&err), [".name"]);
        assert_eq!(err.issues[0].code, IssueCode::MissingField);
        assert_eq!(err.issues[0].message, "Required field is missing or null");
    }
    assert!(s.parse_value(&json!({"name": "a", "age": 1})).is_ok());
}

#[test]
#[allow(deprecated)]
fn deep_partial_is_shallow_partial() {
    let inner = vld::object().field("city", vld::string());
    let s = vld::object()
        .field("name", vld::string().min(1))
        .field("addr", inner)
        .deep_partial();
    assert!(s.parse_value(&json!({})).is_ok());
    // Not recursive: the nested object keeps its required field.
    assert_issue!(
        parse_err(&s, json!({"addr": {}})),
        ".addr.city",
        IssueCode::InvalidType { .. }
    );
}

#[test]
fn field_optional_absent_null_present() {
    let s = vld::object()
        .field("name", vld::string())
        .field_optional("age", vld::number().int().min(0));
    for input in [json!({"name": "Bob"}), json!({"name": "Bob", "age": null})] {
        assert_eq!(
            s.parse_value(&input).unwrap()["age"],
            json!(null),
            "{input}"
        );
    }
    assert_eq!(
        s.parse_value(&json!({"name": "Bob", "age": 30})).unwrap()["age"],
        json!(30)
    );
    assert_issue!(
        parse_err(&s, json!({"name": "Bob", "age": -5})),
        ".age",
        IssueCode::TooSmall { .. }
    );
}

#[test]
fn field_optional_keeps_default() {
    let s = vld::object().field_optional("n", vld::number().with_default(1.0));
    assert_eq!(s.parse_value(&json!({})).unwrap()["n"], json!(1.0));
    assert_eq!(s.parse_value(&json!({"n": 2})).unwrap()["n"], json!(2.0));
}
