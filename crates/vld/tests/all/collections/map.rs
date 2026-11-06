use crate::common::{codes, parse_err, paths};
use serde_json::json;
use vld::error::IssueCode;
use vld::prelude::*;

#[test]
fn pairs_become_a_map() {
    let m = vld::map(vld::string(), vld::number().int());
    let out = m.parse(r#"[["a", 1], ["b", 2]]"#).unwrap();
    assert_eq!(out.len(), 2);
    assert_eq!((out["a"], out["b"]), (1, 2));
    assert!(m.parse("[]").unwrap().is_empty());
    assert_eq!(m.key_schema().parse_value(&json!("k")).unwrap(), "k");
    assert_eq!(m.value_schema().parse_value(&json!(3)).unwrap(), 3);
}

#[test]
fn entry_errors_point_at_the_entry() {
    let m = vld::map(vld::string().min(3), vld::number().int().positive());
    let err = parse_err(
        &m,
        json!([["abc", 1], ["ab", 1], ["xyz", -1], "bad", ["a"]]),
    );
    assert_eq!(paths(&err), ["[1]", "[2]", "[3]", "[4]"]);
    assert_eq!(
        codes(&err),
        [
            "too_small",
            "too_small",
            "invalid_map_entry",
            "invalid_map_entry"
        ]
    );
    assert_issue!(
        parse_err(&m, json!({"a": 1})),
        "",
        IssueCode::InvalidType { .. }
    );
}

#[test]
fn duplicate_key_is_an_error() {
    let m = vld::map(vld::string(), vld::number());
    let err = m.parse(r#"[["a", 1], ["a", 2]]"#).unwrap_err();
    assert_eq!(err.issues.len(), 1);
    assert_issue!(&err, "[1]", IssueCode::DuplicateKey);
    assert_eq!(err.issues[0].received, Some(json!("a")));
}
