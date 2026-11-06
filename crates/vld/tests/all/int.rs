//! `vld::number().int()` (i64): exactness, bounds, coerce, messages.

use crate::common::{code, codes, parse_err};
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

// ---- sized integers (u8 .. isize) ----

#[test]
fn sized_int_ranges() {
    assert_eq!(vld::u8().parse("255").unwrap(), 255u8);
    assert_eq!(
        code(vld::u8().parse("256")),
        IssueCode::TooBig {
            origin: SizeOrigin::Number,
            maximum: json!(255),
            inclusive: true
        }
    );
    assert_eq!(
        code(vld::u8().parse("-1")),
        IssueCode::TooSmall {
            origin: SizeOrigin::Number,
            minimum: json!(0),
            inclusive: true
        }
    );
    assert_eq!(vld::i8().parse("-128").unwrap(), i8::MIN);
    assert!(vld::i8().parse("128").is_err());
    assert_eq!(vld::i16().parse("-32768").unwrap(), i16::MIN);
    assert!(vld::u16().parse("65536").is_err());
    assert_eq!(vld::i32().parse("2147483647").unwrap(), i32::MAX);
    assert!(vld::i32().parse("2147483648").is_err());
    assert_eq!(vld::u32().parse("4294967295").unwrap(), u32::MAX);
    assert_eq!(vld::i64().parse("-9223372036854775808").unwrap(), i64::MIN);
    assert!(vld::i64().parse("9223372036854775808").is_err());
    assert_eq!(vld::usize().parse("7").unwrap(), 7usize);
    assert_eq!(vld::isize().parse("-7").unwrap(), -7isize);
    assert!(vld::usize().parse("-7").is_err());
}

#[test]
fn u64_above_i64_max_is_exact() {
    assert_eq!(vld::u64().parse("18446744073709551615").unwrap(), u64::MAX);
    assert_eq!(
        vld::u64().parse("9223372036854775808").unwrap(),
        9_223_372_036_854_775_808u64
    );
    let err = vld::u64().parse("-1").unwrap_err();
    assert!(err.issues[0].message.contains("at least 0"), "{err}");
    // Beyond u64: serde_json reads it as a float, still out of range.
    assert!(vld::u64().parse("18446744073709551616").is_err());
    // Bounds keep full precision.
    let big = vld::u64().min(u64::MAX - 1);
    assert!(big.parse("18446744073709551615").is_ok());
    assert!(big.parse("18446744073709551613").is_err());
}

#[test]
fn sized_int_floats_and_types() {
    assert_eq!(vld::i32().parse("2.0").unwrap(), 2);
    assert_eq!(code(vld::i32().parse("2.5")), IssueCode::NotInt);
    assert_eq!(code(vld::u8().parse("1e3")), code(vld::u8().parse("1000")));
    assert_eq!(code(vld::i16().parse(r#""5""#)).key(), "invalid_type");
    assert_eq!(code(vld::i16().parse("true")).key(), "invalid_type");
    let msg = vld::u8()
        .type_error("byte please")
        .parse("null")
        .unwrap_err();
    assert_eq!(msg.issues[0].message, "byte please");
    let msg = vld::u8().int_error("whole!").parse("1.5").unwrap_err();
    assert_eq!(msg.issues[0].message, "whole!");
}

#[test]
fn sized_int_coerce() {
    assert_eq!(vld::u16().coerce().parse(r#""443""#).unwrap(), 443);
    assert_eq!(vld::u8().coerce().parse("true").unwrap(), 1);
    assert_eq!(vld::i8().coerce().parse(r#""-3.0""#).unwrap(), -3);
    assert!(vld::u8().coerce().parse(r#""300""#).is_err());
    assert!(vld::u8().coerce().parse(r#""1.5""#).is_err());
    assert!(vld::u8().coerce().parse(r#""abc""#).is_err());
    assert_eq!(
        vld::u64()
            .coerce()
            .parse(r#""18446744073709551615""#)
            .unwrap(),
        u64::MAX
    );
}

#[test]
fn sized_int_builders() {
    let s = vld::i32().min(-5).max(5);
    assert!(s.parse("-5").is_ok() && s.parse("5").is_ok());
    assert!(s.parse("6").is_err() && s.parse("-6").is_err());
    let s = vld::i32().gt(0).lt(10);
    assert!(s.parse("0").is_err() && s.parse("10").is_err() && s.parse("9").is_ok());
    assert!(vld::i32().gte(1).lte(1).parse("1").is_ok());
    assert!(vld::i16().positive().parse("0").is_err());
    assert!(vld::i16().negative().parse("-1").is_ok());
    assert!(vld::i16().non_negative().parse("-1").is_err());
    assert!(vld::i16().non_positive().parse("0").is_ok());
    assert!(vld::i64().safe().parse("9007199254740992").is_err());
    assert!(vld::u32().multiple_of(5).parse("25").is_ok());
    assert_eq!(
        code(vld::u32().multiple_of(5).parse("26")),
        IssueCode::NotMultipleOf { divisor: json!(5) }
    );
    assert!(vld::u32().multiple_of(0).parse("0").is_err());
    let err = vld::u8()
        .max(10)
        .with_messages(|k| (k == "too_big").then(|| "too many".to_string()))
        .parse("11")
        .unwrap_err();
    assert_eq!(err.issues[0].message, "too many");
    // Type range is checked before user bounds: one issue only.
    assert_eq!(vld::u8().max(10).parse("300").unwrap_err().issues.len(), 1);
}

#[test]
fn number_int_is_unchanged() {
    let n: i64 = vld::number().int().parse("42").unwrap();
    assert_eq!(n, 42);
    let _: ZInt = vld::number().int();
    let _: ZInteger<i64> = vld::number().int().min(1);
    assert!(vld::number().int().parse("9223372036854775808").is_err());
}

#[test]
fn integer_reports_the_strictest_bound() {
    let err = vld::u8().max(150).parse_value(&json!(300)).unwrap_err();
    assert!(err.issues[0].message.contains("150"), "{err}");
    let err = vld::u8().min(10).parse_value(&json!(-5)).unwrap_err();
    assert!(err.issues[0].message.contains("10"), "{err}");
    // Without a user bound the type range is reported.
    let err = vld::u8().parse_value(&json!(300)).unwrap_err();
    assert!(err.issues[0].message.contains("255"), "{err}");
}
