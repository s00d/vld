use crate::common::{Role, code, parse_err, paths};
use serde_json::json;
use std::collections::{BTreeMap, HashMap};
use vld::error::{IssueCode, SizeOrigin, StringValidation};
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

#[test]
fn record_with_key_schema() {
    let s = vld::record_with_keys(vld::from_str::<u32>(), vld::string());
    let m: HashMap<u32, String> = s.parse(r#"{"1": "a", "20": "b"}"#).unwrap();
    assert_eq!(m[&20], "b");

    let err = s.parse(r#"{"x": "a", "2": 3}"#).unwrap_err();
    let paths: Vec<_> = err.issues.iter().map(|i| i.path.clone()).collect();
    assert!(paths.contains(&vec![PathSegment::Field("x".into())]));
    assert!(paths.contains(&vec![PathSegment::Field("2".into())]));

    // Bad key and bad value under the same key: both reported.
    let err = s.parse(r#"{"x": 1}"#).unwrap_err();
    assert_eq!(err.issues.len(), 2);

    let uuid_keys = vld::record_with_keys(vld::string().uuid(), vld::boolean());
    assert!(
        uuid_keys
            .parse(r#"{"67e55044-10b1-426f-9247-bb680e5fe0c8": true}"#)
            .is_ok()
    );
    let err = uuid_keys.parse(r#"{"nope": true}"#).unwrap_err();
    assert_eq!(
        err.issues[0].code,
        IssueCode::InvalidString {
            validation: StringValidation::Uuid
        }
    );

    let roles = vld::record_with_keys(vld::from_serde::<Role>(), vld::number().int())
        .min_keys(1)
        .max_keys(2);
    let m = roles.parse(r#"{"admin": 1, "user": 2}"#).unwrap();
    assert_eq!(m[&Role::User], 2);
    assert_eq!(code(roles.parse("{}")).key(), "too_small");
    assert_eq!(code(roles.parse("[]")).key(), "invalid_type");
}

#[test]
fn ordered_records() {
    let m: BTreeMap<String, f64> = vld::record(vld::number())
        .ordered()
        .parse(r#"{"b": 2, "a": 1, "c": 3}"#)
        .unwrap();
    assert_eq!(
        m.keys().map(String::as_str).collect::<Vec<_>>(),
        ["a", "b", "c"]
    );

    let s: ZOrderedRecord<ZBoolean, vld::collections::KeySchema<vld::primitives::ZFromStr<i32>>> =
        vld::record_with_keys(vld::from_str::<i32>(), vld::boolean())
            .max_keys(3)
            .ordered();
    let m = s.parse(r#"{"10": true, "-1": false, "2": true}"#).unwrap();
    assert_eq!(m.keys().copied().collect::<Vec<_>>(), [-1, 2, 10]);
    assert!(
        s.parse(r#"{"1": true, "2": true, "3": true, "4": true}"#)
            .is_err()
    );

    // Default record output is unchanged.
    let _: HashMap<String, bool> = vld::record(vld::boolean()).parse("{}").unwrap();
}
