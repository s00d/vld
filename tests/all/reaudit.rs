//! Regression tests for the 0.5 re-audit findings (one test per finding,
//! numbered as in the audit).

use serde_json::json;
use vld::prelude::*;

// 1. `multiple_of(-1)` on a value saturated to `i128::MIN` overflowed `%`.
#[test]
fn f01_multiple_of_minus_one_never_panics() {
    let s = vld::i64().multiple_of(-1);
    assert!(s.parse_value(&json!(-1e40)).is_err());
    let s = vld::i64().multiple_of(-1).coerce();
    assert!(s
        .parse_value(&json!("-170141183460469231731687303715884105728"))
        .is_err());
    assert_eq!(s.parse_value(&json!(-7)).unwrap(), -7);
}

// 2 + 3. Range edges: no panic, and all date backends agree.
#[cfg(any(feature = "chrono", feature = "jiff", feature = "time"))]
macro_rules! edge_suite {
    ($backend:ident) => {
        mod $backend {
            use serde_json::json;
            use vld::prelude::*;

            #[test]
            fn f02_f03_datetime_range_edges_agree() {
                let s = vld::$backend::datetime();
                for bad in [
                    "9999-12-31T23:00:00-05:00",
                    "9999-12-31T00:00:00Z",
                    "9999-12-30T22:00:01Z",
                    "0000-01-01T00:30:00+01:00",
                ] {
                    assert!(s.parse_value(&json!(bad)).is_err(), "{bad}");
                }
                for ok in [
                    "9999-12-30T22:00:00.999999999Z",
                    "9999-12-31T00:00:00+02:00",
                    "0000-01-01T00:00:00Z",
                    "0000-01-01T00:30:00-01:00",
                ] {
                    assert!(s.parse_value(&json!(ok)).is_ok(), "{ok}");
                }
                // Naive input shifted past either end by the naive offset.
                let n = vld::$backend::datetime().naive_timezone_offset(-5 * 3600);
                assert!(n.parse_value(&json!("9999-12-31T23:00:00")).is_err());
                let n = vld::$backend::datetime().naive_timezone_offset(3600);
                assert!(n.parse_value(&json!("0000-01-01T00:00:00")).is_err());
            }
        }
    };
}
#[cfg(feature = "chrono")]
edge_suite!(chrono);
#[cfg(feature = "jiff")]
edge_suite!(jiff);
#[cfg(feature = "time")]
edge_suite!(time);

// 4. f32 bounds work at f32 precision and print the short form.
#[test]
fn f04_f32_multiple_of_and_messages() {
    let s = vld::f32().multiple_of(0.1);
    assert_eq!(s.parse_value(&json!(0.3)).unwrap(), 0.3f32);
    let err = s.parse_value(&json!(0.35)).unwrap_err();
    assert_eq!(err.issues[0].message, "Number must be a multiple of 0.1");
    assert_eq!(vld::f32().max(0.1).parse_value(&json!(0.1)).unwrap(), 0.1f32);
    let err = vld::f32().min(0.1).parse_value(&json!(0.05)).unwrap_err();
    assert_eq!(err.issues[0].message, "Number must be at least 0.1");
}

// 5. `union!` with 7 and 13 schemas compiles and keeps first-match order.
#[test]
fn f05_union_macro_7_and_13() {
    let s7 = vld::union!(
        vld::literal(1),
        vld::literal(2),
        vld::literal(3),
        vld::literal(4),
        vld::literal(5),
        vld::literal(6),
        vld::literal(7),
    );
    assert!(s7.parse("7").is_ok());
    assert!(s7.parse("8").is_err());
    let s13 = vld::union!(
        vld::literal(1),
        vld::literal(2),
        vld::literal(3),
        vld::literal(4),
        vld::literal(5),
        vld::literal(6),
        vld::literal(7),
        vld::literal(8),
        vld::literal(9),
        vld::literal(10),
        vld::literal(11),
        vld::literal(12),
        vld::literal(13)
    );
    assert!(s13.parse("13").is_ok());
    // 19 (unions): nested unions are flattened, prefixes never stack.
    let err = s13.parse("14").unwrap_err();
    assert_eq!(err.issues[0].code, IssueCode::InvalidUnion);
    let msgs: Vec<&str> = err.issues[1..].iter().map(|i| i.message.as_str()).collect();
    assert_eq!(msgs.len(), 13);
    for (i, m) in msgs.iter().enumerate() {
        assert_eq!(*m, format!("[variant {}] Expected literal {}", i + 1, i + 1));
    }
}

// 22. Discriminator values compare numerically (1 == 1.0).
#[test]
fn f22_discriminator_numeric_equality() {
    let s = vld::discriminated_union("v").variant(1, vld::object().field("v", vld::number()));
    assert!(s.parse(r#"{"v": 1.0}"#).is_ok());
    assert!(s.parse(r#"{"v": 1}"#).is_ok());
    assert!(s.parse(r#"{"v": 2}"#).is_err());
}

// 35. Literals of every integer type and f32.
#[test]
fn f35_literal_int_types() {
    assert_eq!(vld::literal(5u8).parse("5").unwrap(), 5u8);
    assert_eq!(vld::literal(5i32).parse("5.0").unwrap(), 5i32);
    assert_eq!(
        vld::literal(u64::MAX).parse("18446744073709551615").unwrap(),
        u64::MAX
    );
    assert!(vld::literal(5u16).parse("6").is_err());
    assert_eq!(vld::literal(-3isize).parse("-3").unwrap(), -3isize);
    assert_eq!(vld::literal(0.1f32).parse("0.1").unwrap(), 0.1f32);
}
