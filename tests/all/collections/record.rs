use crate::common::{parse_err, paths};
use serde_json::json;
use vld::error::{IssueCode, SizeOrigin};
use vld::prelude::*;

#[test]
fn values_are_validated_with_key_paths() {
    let r = vld::record(vld::number().int().positive());
    let out = r.parse_value(&json!({"a": 1, "b": 2, "c": 3})).unwrap();
    assert_eq!(out.len(), 3);
    assert_eq!(out.get("b"), Some(&2));

    let err = parse_err(&r, json!({"a": 1, "b": -1, "é": "x"}));
    let mut p = paths(&err);
    p.sort();
    assert_eq!(p, [".b", ".é"]);
    assert_issue!(parse_err(&r, json!([1])), "", IssueCode::InvalidType { expected, .. } if expected == "object");
}

#[test]
fn key_count_bounds() {
    let r = vld::record(vld::string()).min_keys(1).max_keys(2);
    assert!(r.parse_value(&json!({"a": "1"})).is_ok());
    assert_issue!(
        parse_err(&r, json!({})),
        "",
        IssueCode::TooSmall {
            origin: SizeOrigin::Record,
            ..
        }
    );
    assert_issue!(
        parse_err(&r, json!({"a": "1", "b": "2", "c": "3"})),
        "",
        IssueCode::TooBig {
            origin: SizeOrigin::Record,
            ..
        }
    );
}
