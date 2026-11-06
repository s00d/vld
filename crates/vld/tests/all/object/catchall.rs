use crate::common::{parse_err, paths};
use serde_json::json;
use vld::error::IssueCode;
use vld::prelude::*;

#[test]
fn catchall_validates_unknown_fields() {
    let s = vld::object()
        .field("name", vld::string())
        .catchall(vld::number().int());
    let out = s.parse(r#"{"name":"Alex","score":42}"#).unwrap();
    assert_eq!(out["score"], json!(42));

    let err = parse_err(&s, json!({"name": "Alex", "score": "nope", "rank": 1.5}));
    let mut p = paths(&err);
    p.sort();
    assert_eq!(p, [".rank", ".score"]);
}

/// With a catchall, `strict()` no longer rejects unknown keys.
#[test]
fn catchall_overrides_unknown_mode() {
    let s = vld::object()
        .field("a", vld::string())
        .strict()
        .catchall(vld::string());
    let out = s.parse(r#"{"a":"x","b":"y"}"#).unwrap();
    assert_eq!(out["b"], json!("y"));
    assert_issue!(
        s.parse(r#"{"a":"x","b":1}"#),
        ".b",
        IssueCode::InvalidType { .. }
    );
}
