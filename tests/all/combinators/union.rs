use crate::common::parse_err;
use serde_json::json;
use vld::error::IssueCode;
use vld::prelude::*;

#[test]
fn union2_returns_the_matching_side() {
    let s = vld::union(vld::string(), vld::number().int());
    assert_eq!(
        s.parse_value(&json!("hello")).unwrap().left().unwrap(),
        "hello"
    );
    assert_eq!(s.parse_value(&json!(42)).unwrap().right().unwrap(), 42);
    // `.or()` builds the same union.
    let or = vld::string().or(vld::number().int());
    assert!(or.parse("\"x\"").unwrap().is_left());
    assert!(or.parse("1").unwrap().is_right());
    assert_issue!(or.parse("true"), "", IssueCode::InvalidUnion);
}

#[test]
fn union3_and_macro() {
    let s = vld::union3(vld::string(), vld::number().int(), vld::boolean());
    for ok in [json!("hi"), json!(42), json!(true)] {
        assert!(s.parse_value(&ok).is_ok(), "{ok}");
    }
    assert_issue!(parse_err(&s, json!(null)), "", IssueCode::InvalidUnion);

    let many = vld::union!(
        vld::literal("a"),
        vld::literal("b"),
        vld::literal("c"),
        vld::literal("d"),
        vld::literal("e"),
        vld::literal("f"),
        vld::literal("g"),
        vld::literal("h"),
    );
    assert!(many.parse_value(&json!("h")).is_ok());
    assert!(many.parse_value(&json!("z")).is_err());
}

#[test]
fn literal_union() {
    let s = vld::union(vld::literal("hello"), vld::literal(42i64));
    assert!(s.parse_value(&json!("hello")).is_ok());
    assert!(s.parse_value(&json!(42)).is_ok());
    assert!(s.parse_value(&json!("world")).is_err());
}

/// The first issue is `invalid_union` (with the received value), followed by
/// every variant's own issues, tagged `[variant N]`.
#[test]
fn error_keeps_variant_issues() {
    let u = vld::union(vld::string().min(5), vld::number().min(10.0));
    let err = u.parse("3").unwrap_err();
    assert_eq!(err.issues[0].code, IssueCode::InvalidUnion);
    assert_eq!(err.issues[0].received, Some(json!(3)));
    assert_eq!(err.issues.len(), 3, "{err}");
    assert!(
        err.issues[1].message.starts_with("[variant 1]"),
        "{}",
        err.issues[1].message
    );
    assert!(
        err.issues[2].message.starts_with("[variant 2]"),
        "{}",
        err.issues[2].message
    );
    assert_eq!(err.issues[1].code.key(), "invalid_type");
    assert_eq!(err.issues[2].code.key(), "too_small");
}

#[test]
fn accessors_expose_the_variants() {
    let u = vld::union3(vld::string().min(2), vld::number().int(), vld::boolean());
    assert!(u.schema_a().parse_value(&json!("a")).is_err());
    assert_eq!(u.schema_b().parse_value(&json!(3)).unwrap(), 3);
    assert!(u.schema_c().parse_value(&json!(true)).unwrap());
    let u2 = vld::union(vld::string(), vld::number());
    assert_eq!(u2.schema_a().parse_value(&json!("x")).unwrap(), "x");
    assert_eq!(u2.schema_b().parse_value(&json!(1.5)).unwrap(), 1.5);
}
