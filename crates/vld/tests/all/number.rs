//! `vld::number()` (f64): bounds, sign, multiple_of, safe, finite, coerce.

use crate::common::{code, codes, parse_err};
use serde_json::json;
use vld::error::{IssueCode, SizeOrigin};
use vld::prelude::*;

#[test]
fn accepts_numbers_only() {
    let n = vld::number();
    assert_eq!(n.parse_value(&json!(42.5)).unwrap(), 42.5);
    assert_eq!(n.parse_value(&json!(-3)).unwrap(), -3.0);
    for bad in [json!("1"), json!(null), json!(true), json!([1])] {
        assert_issue!(parse_err(&n, bad), "", IssueCode::InvalidType { expected, .. } if expected == "number");
    }
}

/// `(schema, input, expected code key or None)` at and around every bound.
#[test]
fn bound_table() {
    let cases: Vec<(ZNumber, f64, Option<&str>)> = vec![
        (vld::number().min(0.0), 0.0, None),
        (vld::number().min(0.0), -0.0001, Some("too_small")),
        (vld::number().gte(0.0), 0.0, None),
        (vld::number().gt(0.0), 0.0, Some("too_small")),
        (vld::number().gt(0.0), 1e-9, None),
        (vld::number().max(1.0), 1.0, None),
        (vld::number().max(1.0), 1.0001, Some("too_big")),
        (vld::number().lte(1.0), 1.0, None),
        (vld::number().lt(1.0), 1.0, Some("too_big")),
        (vld::number().lt(1.0), 0.999, None),
        (vld::number().positive(), 0.0, Some("too_small")),
        (vld::number().positive(), 0.1, None),
        (vld::number().negative(), 0.0, Some("too_big")),
        (vld::number().negative(), -0.1, None),
        (vld::number().non_negative(), 0.0, None),
        (vld::number().non_negative(), -1.0, Some("too_small")),
        (vld::number().non_positive(), 0.0, None),
        (vld::number().non_positive(), 1.0, Some("too_big")),
    ];
    for (schema, input, want) in cases {
        let got = schema.parse_value(&json!(input));
        match want {
            None => assert_eq!(got.unwrap(), input),
            Some(code) => assert_eq!(codes(&got.unwrap_err()), [code], "{input}"),
        }
    }
}

#[test]
fn bound_codes_carry_bound_and_inclusiveness() {
    assert_issue!(
        parse_err(&vld::number().gt(5.0), json!(5)),
        "",
        IssueCode::TooSmall { origin: SizeOrigin::Number, minimum, inclusive: false } if minimum == &json!(5.0)
    );
    assert_issue!(
        parse_err(&vld::number().lte(5.0), json!(6)),
        "",
        IssueCode::TooBig { origin: SizeOrigin::Number, maximum, inclusive: true } if maximum == &json!(5.0)
    );
    // Both bounds failing on different values report one issue each.
    let s = vld::number().min(0.0).max(100.0);
    assert_eq!(codes(&parse_err(&s, json!(-1))), ["too_small"]);
    assert_eq!(codes(&parse_err(&s, json!(101))), ["too_big"]);
}

#[test]
fn multiple_of_with_float_tolerance() {
    let s = vld::number().multiple_of(0.1);
    for ok in [0.3, -0.3, 0.0, 1.1] {
        assert!(s.parse_value(&json!(ok)).is_ok(), "{ok}");
    }
    assert_issue!(
        parse_err(&s, json!(0.35)),
        "",
        IssueCode::NotMultipleOf { divisor } if divisor == &json!(0.1)
    );
    // A zero divisor never matches.
    assert_issue!(
        parse_err(&vld::number().multiple_of(0.0), json!(5)),
        "",
        IssueCode::NotMultipleOf { .. }
    );
}

#[test]
fn safe_range() {
    let s = vld::number().safe();
    for ok in [42.0, 9007199254740991.0, -9007199254740991.0] {
        assert!(s.parse_value(&json!(ok)).is_ok(), "{ok}");
    }
    for bad in [json!(9007199254740992_i64), json!(-9007199254740992_i64)] {
        assert_issue!(parse_err(&s, bad), "", IssueCode::NotSafeInteger);
    }
}

#[test]
fn coerce_parses_strings_and_rejects_non_finite() {
    let s = vld::number().coerce();
    assert_eq!(s.parse_value(&json!("2.5")).unwrap(), 2.5);
    assert_eq!(s.parse_value(&json!(3)).unwrap(), 3.0);
    for bad in ["NaN", "inf", "-infinity", "1e309", "abc", "", "١"] {
        assert!(s.parse_value(&json!(bad)).is_err(), "{bad:?}");
    }
}

#[test]
fn messages() {
    let s = vld::number().type_error("Numbers only!");
    assert_eq!(parse_err(&s, json!("x")).issues[0].message, "Numbers only!");

    let s = vld::number()
        .min(0.0)
        .max(100.0)
        .with_messages(|key| match key {
            "too_small" => Some("Must be >= 0".into()),
            "too_big" => Some("Must be <= 100".into()),
            _ => None,
        });
    assert_eq!(parse_err(&s, json!(-5)).issues[0].message, "Must be >= 0");
    assert_eq!(
        parse_err(&s, json!(200)).issues[0].message,
        "Must be <= 100"
    );
    assert_eq!(parse_err(&s, json!(200)).issues[0].code.key(), "too_big");
}

// ---- f32 ----

#[test]
fn f32_range_and_bounds() {
    assert_eq!(vld::f32().parse("1.5").unwrap(), 1.5f32);
    assert_eq!(
        code(vld::f32().parse("1e39")),
        IssueCode::TooBig {
            origin: SizeOrigin::Number,
            maximum: json!(f32::MAX as f64),
            inclusive: true
        }
    );
    assert_eq!(code(vld::f32().parse("-1e39")).key(), "too_small");
    assert_eq!(
        vld::f32().parse_value(&json!(f32::MAX as f64)).unwrap(),
        f32::MAX
    );
    assert_eq!(vld::f32().parse("3.4028235e38").unwrap(), f32::MAX);
    assert_eq!(vld::f32().parse("-3.4028235e38").unwrap(), f32::MIN);
    // Bounds compare in f32 space: 0.1 passes min(0.1) / max(0.1).
    assert!(vld::f32().min(0.1).max(0.1).parse("0.1").is_ok());
    assert!(vld::f32().gt(0.0).parse("0").is_err());
    assert!(vld::f32().lt(1.0).parse("1").is_err());
    assert!(vld::f32().positive().parse("-0.5").is_err());
    assert!(vld::f32().coerce().parse(r#""2.5""#).is_ok());
    assert!(vld::f32().parse(r#""2.5""#).is_err());
    assert!(vld::f32().multiple_of(0.5).parse("2.5").is_ok());
    assert!(vld::f32().non_negative().non_positive().parse("0").is_ok());
    assert!(vld::f32().negative().parse("0").is_err());
    assert!(vld::f32().gte(1.0).lte(2.0).parse("1.5").is_ok());
    let err = vld::f32().type_error("float!").parse("null").unwrap_err();
    assert_eq!(err.issues[0].message, "float!");
}
