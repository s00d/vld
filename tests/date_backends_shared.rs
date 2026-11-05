//! Same accept/reject table for every enabled date backend (each backend
//! gets its own module, so all three can be enabled at once).
#![cfg(any(feature = "chrono", feature = "jiff", feature = "time"))]

macro_rules! backend_tests {
    ($backend:ident) => {
        mod $backend {
            use vld::prelude::*;
            use vld::$backend::{date, datetime};

            #[test]
            fn date_accepts_only_strict_iso() {
                let s = date();
                for ok in ["2024-06-15", "2024-02-29", "0001-01-01"] {
                    assert!(s.parse_value(&serde_json::json!(ok)).is_ok(), "{}", ok);
                }
                for bad in [
                    "2024-6-5",
                    "2024-06-5",
                    "2023-02-29",
                    "2024-02-31",
                    "2024-06-15T00:00:00Z",
                    " 2024-06-15",
                    "+2024-06-15",
                    "2024-06-15é",
                ] {
                    assert!(s.parse_value(&serde_json::json!(bad)).is_err(), "{}", bad);
                }
            }

            #[test]
            fn datetime_accepts_only_strict_rfc3339() {
                let s = datetime();
                for ok in [
                    "2024-06-15T12:00:00Z",
                    "2024-06-15T12:00:00+03:00",
                    "2024-06-15T12:00:00.123456789-05:30",
                    "2024-06-15t12:00:00z",
                    "2024-06-15T12:00:00",
                    "2024-06-15T12:00:00.5",
                ] {
                    assert!(s.parse_value(&serde_json::json!(ok)).is_ok(), "{}", ok);
                }
                for bad in [
                    "2024-06-15",
                    "2024-06-15T12:00",
                    "2024-06-15T12:00Z",
                    "2024-06-15 12:00:00Z",
                    "2024-6-15T12:00:00Z",
                    "2024-06-15T12:00:00+0300",
                    "2024-06-15T12:00:00Zjunk",
                    "2024-06-15T12:00:00.Z",
                    "2024-06-15T12:00:00.1234567890Z",
                    "2024-06-15T24:00:00Z",
                    "2024-06-15T12:00:00[UTC]",
                ] {
                    assert!(s.parse_value(&serde_json::json!(bad)).is_err(), "{}", bad);
                }
            }

            #[test]
            fn timezone_offset_only_rejects_naive_input() {
                let s = datetime().timezone_offset_only(3 * 3600);
                assert!(s.parse(r#""2024-06-15T12:30:00+03:00""#).is_ok());
                let err = s.parse(r#""2024-06-15T12:30:00""#).unwrap_err();
                assert_eq!(err.issues[0].code.key(), "invalid_timezone_offset");
                assert!(err.issues[0].message.contains("+03:00"));
            }

            #[test]
            fn date_bounds_report_real_bound() {
                let err = date()
                    .min("2024-01-01")
                    .parse(r#""2023-01-01""#)
                    .unwrap_err();
                assert_eq!(
                    err.issues[0].code,
                    vld::error::IssueCode::TooSmall {
                        origin: vld::error::SizeOrigin::Date,
                        minimum: serde_json::json!("2024-01-01"),
                        inclusive: true,
                    }
                );
                assert!(err.issues[0].message.contains("2024-01-01"));
                let tr = vld::i18n::translate_error(&err, &vld::i18n::english());
                assert!(tr.issues[0].message.contains("2024-01-01"));

                let err = datetime()
                    .max("2024-01-01T00:00:00Z")
                    .parse(r#""2025-01-01T00:00:00Z""#)
                    .unwrap_err();
                match &err.issues[0].code {
                    vld::error::IssueCode::TooBig { origin, .. } => {
                        assert_eq!(*origin, vld::error::SizeOrigin::DateTime)
                    }
                    other => panic!("unexpected code {:?}", other),
                }
                assert!(err.issues[0].message.contains("2024-01-01"));
            }

            /// date/time outputs stay usable with `with_default` + JSON Schema.
            #[cfg(feature = "openapi")]
            #[test]
            fn with_default_date_emits_default() {
                use vld::openapi::JsonSchema;
                let d = date().parse(r#""2024-01-02""#).unwrap();
                let js = date().with_default(d).json_schema();
                assert_eq!(js["default"], "2024-01-02");
                assert_eq!(js["format"], "date");
                let dt = datetime().parse(r#""2024-01-02T03:04:05Z""#).unwrap();
                let js = datetime().with_default(dt).json_schema();
                assert!(js["default"].is_string());
                assert_eq!(js["format"], "date-time");
            }
        }
    };
}

#[cfg(feature = "chrono")]
backend_tests!(chrono);
#[cfg(feature = "jiff")]
backend_tests!(jiff);
#[cfg(feature = "time")]
backend_tests!(time);

/// With several backends enabled, each module keeps its own output type.
#[cfg(all(feature = "chrono", feature = "jiff", feature = "time"))]
#[test]
fn all_backends_coexist() {
    use vld::prelude::*;
    let c: ::chrono::NaiveDate = vld::chrono::date().parse(r#""2024-06-15""#).unwrap();
    let j: ::jiff::civil::Date = vld::jiff::date().parse(r#""2024-06-15""#).unwrap();
    let t: ::time::Date = vld::time::date().parse(r#""2024-06-15""#).unwrap();
    assert_eq!(c.to_string(), j.to_string());
    assert_eq!(j.to_string(), t.to_string());
}

/// The `vld::date()` shortcut exists only with exactly one backend.
#[cfg(all(feature = "chrono", not(feature = "jiff"), not(feature = "time")))]
#[test]
fn single_backend_shortcut() {
    use vld::prelude::*;
    let d: ::chrono::NaiveDate = vld::date().parse(r#""2024-06-15""#).unwrap();
    let _: ZDate = vld::date();
    assert_eq!(d.to_string(), "2024-06-15");
}
