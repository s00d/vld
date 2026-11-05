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
