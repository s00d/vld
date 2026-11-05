//! Shape operations: `pick`, `omit`, `extend`/`merge`, `when`.

use crate::common::{parse_err, paths};
use serde_json::json;
use vld::error::IssueCode;
use vld::prelude::*;

fn abc() -> vld::object::ZObject {
    vld::object()
        .field("a", vld::string())
        .field("b", vld::number())
        .field("c", vld::boolean())
}

/// The dropped key, when present, is unknown: stripped by default, rejected
/// under `strict()`, and no longer validated.
#[test]
fn pick_drops_other_fields() {
    let input = json!({"a": "x", "b": "not a number", "c": true});
    let out = abc().pick(&["a", "c"]).parse_value(&input).unwrap();
    assert_eq!(out.len(), 2);
    assert!(out.get("b").is_none());

    let err = parse_err(&abc().strict().pick(&["a", "c"]), input);
    assert_eq!(paths(&err), [".b"]);
    assert_eq!(err.issues[0].code, IssueCode::UnrecognizedField);
    assert_eq!(abc().pick(&["c", "a", "zzz"]).keyof(), ["a", "c"]);
}

#[test]
fn omit_drops_one_field() {
    let input = json!({"a": "x", "b": "not a number", "c": true});
    let out = abc().omit("b").parse_value(&input).unwrap();
    assert_eq!(out.len(), 2);
    assert!(out.get("b").is_none());

    let err = parse_err(&abc().strict().omit("b"), input);
    assert_eq!(paths(&err), [".b"]);
    assert_eq!(abc().omit("missing").keyof(), ["a", "b", "c"]);
}

#[test]
fn extend_and_merge_later_schema_wins() {
    let base = vld::object().field("a", vld::string().min(1));
    let extended = base.extend(vld::object().field("b", vld::number()));
    let out = extended.parse_value(&json!({"a": "x", "b": 42})).unwrap();
    assert_eq!(out.len(), 2);

    let merged = vld::object()
        .field("a", vld::string().min(1))
        .merge(vld::object().field("a", vld::string().min(10)));
    assert_eq!(merged.keyof(), ["a"]);
    assert_issue!(
        parse_err(&merged, json!({"a": "hi"})),
        ".a",
        IssueCode::TooSmall { minimum, .. } if minimum == &json!(10)
    );
}

#[test]
fn when_applies_extra_schema_only_on_match() {
    let s = vld::object()
        .field("role", vld::string())
        .field_optional("admin_key", vld::string())
        .when("role", "admin", "admin_key", vld::string().min(10));

    assert!(s
        .parse(r#"{"role": "admin", "admin_key": "super-secret-key"}"#)
        .is_ok());
    assert!(s.parse(r#"{"role": "user"}"#).is_ok());
    assert!(s.parse(r#"{"role": "user", "admin_key": "x"}"#).is_ok());
    assert_issue!(
        s.parse(r#"{"role": "admin", "admin_key": "short"}"#),
        ".admin_key",
        IssueCode::TooSmall { .. }
    );
    // A missing target is validated as null.
    assert_issue!(
        s.parse(r#"{"role": "admin"}"#),
        ".admin_key",
        IssueCode::InvalidType { .. }
    );

    let n = vld::object()
        .field("type", vld::number().int())
        .field_optional("detail", vld::string())
        .when("type", json!(1), "detail", vld::string().min(5));
    assert_issue!(
        n.parse(r#"{"type": 1, "detail": "ab"}"#),
        ".detail",
        IssueCode::TooSmall { .. }
    );
    assert!(n.parse(r#"{"type": 2, "detail": "ab"}"#).is_ok());
}
