//! Unknown-key handling: strip (default), `strip()`, `passthrough()`, `strict()`.

use crate::common::{codes, parse_err, paths};
use serde_json::json;
use vld::error::{IssueCode, SizeOrigin};
use vld::prelude::*;

fn base() -> vld::object::ZObject {
    vld::object()
        .field("name", vld::string().min(1))
        .field("age", vld::number().int().min(0))
}

#[test]
fn valid_object() {
    let out = base()
        .parse_value(&json!({"name": "Alex", "age": 25}))
        .unwrap();
    assert_eq!(out["name"], json!("Alex"));
    assert_eq!(out["age"], json!(25));
    assert_issue!(parse_err(&base(), json!([])), "", IssueCode::InvalidType { expected, .. } if expected == "object");
}

#[test]
fn field_errors_have_field_paths() {
    let err = parse_err(&base(), json!({"name": "", "age": -1}));
    assert_eq!(paths(&err), [".name", ".age"]);
    // An absent required key is `missing_field` (a present null is `invalid_type`).
    let err = parse_err(&base(), json!({}));
    assert_eq!(paths(&err), [".name", ".age"]);
    assert_eq!(codes(&err), ["missing_field", "missing_field"]);
    let err = parse_err(&base(), json!({"name": null, "age": null}));
    assert_eq!(codes(&err), ["invalid_type", "invalid_type"]);
}

#[test]
fn strip_is_default_and_explicit() {
    let input = json!({"name": "Alex", "age": 1, "extra": true});
    for schema in [base(), base().passthrough().strip()] {
        let out = schema.parse_value(&input).unwrap();
        assert_eq!(out.len(), 2);
        assert!(out.get("extra").is_none());
    }
}

#[test]
fn passthrough_keeps_unknown_unvalidated() {
    let out = base()
        .passthrough()
        .parse_value(&json!({"name": "Alex", "age": 1, "extra": [1, "x"]}))
        .unwrap();
    assert_eq!(out["extra"], json!([1, "x"]));
}

#[test]
fn strict_rejects_each_unknown_key() {
    let err = parse_err(
        &base().strict(),
        json!({"name": "Alex", "age": 1, "x": 1, "é": 2}),
    );
    let mut p = paths(&err);
    p.sort();
    assert_eq!(p, [".x", ".é"]);
    assert!(err
        .issues
        .iter()
        .all(|i| i.code == IssueCode::UnrecognizedField));
    assert_eq!(err.issues[0].message, "Unrecognized field");
}

#[test]
fn min_max_keys_count_input_keys() {
    let s = vld::object()
        .field("name", vld::string())
        .min_keys(1)
        .max_keys(2)
        .passthrough();
    assert!(s.parse_value(&json!({"name": "Alex"})).is_ok());
    assert_issue!(
        parse_err(&s, json!({})),
        "",
        IssueCode::TooSmall {
            origin: SizeOrigin::Object,
            ..
        }
    );
    assert_issue!(
        parse_err(&s, json!({"name": "A", "b": 1, "c": 2})),
        "",
        IssueCode::TooBig {
            origin: SizeOrigin::Object,
            ..
        }
    );
}

#[test]
fn keyof_lists_fields_in_order() {
    assert_eq!(base().keyof(), ["name", "age"]);
    assert!(vld::object().keyof().is_empty());
}
