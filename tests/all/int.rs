//! `vld::number().int()` (i64): exactness, bounds, coerce, messages.

use crate::common::{codes, parse_err};
use serde_json::json;
use vld::error::{IssueCode, SizeOrigin};
use vld::prelude::*;

#[test]
fn integers_only_and_exact() {
    let s = vld::number().int();
    assert_eq!(s.parse_value(&json!(42)).unwrap(), 42);
    assert_eq!(s.parse_value(&json!(-7)).unwrap(), -7);
    assert_eq!(s.parse("9007199254740993").unwrap(), 9007199254740993);
    assert_eq!(s.parse_value(&json!(i64::MAX)).unwrap(), i64::MAX);
    assert_eq!(s.parse_value(&json!(i64::MIN)).unwrap(), i64::MIN);
    assert_issue!(parse_err(&s, json!(42.5)), "", IssueCode::NotInt);
    assert_issue!(
        parse_err(&s, json!("42")),
        "",
        IssueCode::InvalidType { .. }
    );
    // Out of i64 range / not representable.
    assert!(s.parse("18446744073709551615").is_err());
    assert!(s.parse("1e300").is_err());
}

#[test]
fn bound_table() {
    let cases: Vec<(ZInt, i64, Option<&str>)> = vec![
        (vld::number().int().min(0), 0, None),
        (vld::number().int().min(0), -1, Some("too_small")),
        (vld::number().int().max(100), 100, None),
        (vld::number().int().max(100), 101, Some("too_big")),
        (vld::number().int().gt(0), 0, Some("too_small")),
        (vld::number().int().gt(0), 1, None),
        (vld::number().int().gte(0), 0, None),
        (vld::number().int().lt(10), 10, Some("too_big")),
        (vld::number().int().lte(10), 10, None),
        (vld::number().int().positive(), 0, Some("too_small")),
        (vld::number().int().negative(), 0, Some("too_big")),
        (vld::number().int().non_negative(), 0, None),
        (vld::number().int().non_positive(), 0, None),
        (vld::number().int().non_positive(), -3, None),
        (vld::number().int().non_positive(), 1, Some("too_big")),
        (vld::number().int().multiple_of(3), 9, None),
        (
            vld::number().int().multiple_of(3),
            10,
            Some("not_multiple_of"),
        ),
        (
            vld::number().int().multiple_of(0),
            5,
            Some("not_multiple_of"),
        ),
        (vld::number().int().safe(), 9007199254740991, None),
        (
            vld::number().int().safe(),
            9007199254740992,
            Some("not_safe_integer"),
        ),
    ];
    for (schema, input, want) in cases {
        let got = schema.parse_value(&json!(input));
        match want {
            None => assert_eq!(got.unwrap(), input),
            Some(code) => assert_eq!(codes(&got.unwrap_err()), [code], "{input}"),
        }
    }
    // Bounds compare exactly beyond f64 precision.
    let bounded = vld::number().int().max(9007199254740992);
    assert_issue!(
        bounded.parse("9007199254740993"),
        "",
        IssueCode::TooBig {
            origin: SizeOrigin::Number,
            ..
        }
    );
}

#[test]
fn coerce_parses_integer_strings() {
    let s = vld::number().int().coerce();
    assert_eq!(s.parse_value(&json!("42")).unwrap(), 42);
    assert_eq!(
        s.parse_value(&json!("9007199254740993")).unwrap(),
        9007199254740993
    );
    for bad in ["4.5", "abc", "", "٤٢"] {
        assert!(s.parse_value(&json!(bad)).is_err(), "{bad:?}");
    }
}

#[test]
fn messages() {
    let s = vld::number().int().type_error("Integers only!");
    assert_eq!(
        parse_err(&s, json!("x")).issues[0].message,
        "Integers only!"
    );
    let s = vld::number().int().int_error("Whole numbers only!");
    assert_eq!(
        parse_err(&s, json!(3.5)).issues[0].message,
        "Whole numbers only!"
    );

    let s = vld::number()
        .int()
        .min(1)
        .max(10)
        .with_messages(|key| match key {
            "too_small" => Some("Min 1".into()),
            "too_big" => Some("Max 10".into()),
            "not_int" => Some("No decimals".into()),
            _ => None,
        });
    assert_eq!(parse_err(&s, json!(0)).issues[0].message, "Min 1");
    assert_eq!(parse_err(&s, json!(11)).issues[0].message, "Max 10");
    assert_eq!(parse_err(&s, json!(3.5)).issues[0].message, "No decimals");
}
