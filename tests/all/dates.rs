//! `date()` / `datetime()` — one suite, run for every enabled backend
//! (`vld::chrono`, `vld::jiff`, `vld::time`), so all three can be on at once.
//!
//! Backend values are obtained by parsing, which keeps the suite generic.

macro_rules! backend_suite {
    ($backend:ident, $date_ty:ty $(, #[$dt_default:meta])? $(,)?) => {
        mod $backend {
            use crate::common::{codes, parse_err, paths};
            use serde_json::json;
            use vld::error::{DateKind, IssueCode, SizeOrigin};
            use vld::prelude::*;
            use vld::$backend::{date, datetime};

            fn d(s: &str) -> <vld::$backend::ZDate as VldSchema>::Output {
                date().parse_value(&json!(s)).unwrap()
            }

            fn dt(s: &str) -> <vld::$backend::ZDateTime as VldSchema>::Output {
                datetime().parse_value(&json!(s)).unwrap()
            }

            #[test]
            fn date_accepts_only_strict_iso() {
                assert_eq!(d("2024-06-15").to_string(), "2024-06-15");
                for ok in ["2024-02-29", "2000-02-29"] {
                    assert_eq!(d(ok).to_string(), ok);
                }
                for ok in ["0001-01-01", "9999-12-31"] {
                    assert!(date().parse_value(&json!(ok)).is_ok(), "{ok}");
                }
                for bad in [
                    "", "06/15/2024", "2024-6-5", "2024-06-5", "2023-02-29", "1900-02-29",
                    "2024-02-31", "2024-04-31", "2024-13-01", "2024-00-10", "2024-01-00",
                    "2024-06-15T00:00:00Z", "2024-01-01T", " 2024-06-15", "+2024-06-15",
                    "2024-06-15é", "２０２４-06-15", "é",
                ] {
                    assert_issue!(
                        parse_err(&date(), json!(bad)),
                        "",
                        IssueCode::InvalidDate { kind: DateKind::Date }
                    );
                }
                for bad in [json!(42), json!(null), json!(true)] {
                    let err = parse_err(&date(), bad);
                    assert_eq!(codes(&err), ["invalid_type"]);
                    assert!(err.issues[0].message.contains("Expected date string"), "{err}");
                }
                let err = parse_err(&date().type_error("Date required!"), json!(true));
                assert_eq!(err.issues[0].message, "Date required!");
            }

            #[test]
            fn date_bounds_report_the_bound() {
                let s = date().min("2024-01-01").max("2024-12-31");
                for ok in ["2024-01-01", "2024-07-04", "2024-12-31"] {
                    assert!(s.parse_value(&json!(ok)).is_ok(), "{ok}");
                }
                let err = parse_err(&s, json!("2023-12-31"));
                assert_eq!(
                    err.issues[0].code,
                    IssueCode::TooSmall {
                        origin: SizeOrigin::Date,
                        minimum: json!("2024-01-01"),
                        inclusive: true,
                    }
                );
                assert!(err.issues[0].message.contains("2024-01-01"), "{err}");
                let tr = vld::i18n::translate_error(&err, &vld::i18n::english());
                assert!(tr.issues[0].message.contains("2024-01-01"));
                assert_issue!(
                    parse_err(&s, json!("2025-01-01")),
                    "",
                    IssueCode::TooBig { origin: SizeOrigin::Date, maximum, .. } if maximum == &json!("2024-12-31")
                );
            }

            #[test]
            fn date_bounds_from_backend_values() {
                let s = date().min_date(d("2024-01-01")).max_date(d("2024-01-31"));
                assert!(s.parse_value(&json!("2024-01-15")).is_ok());
                assert_issue!(parse_err(&s, json!("2023-12-31")), "", IssueCode::TooSmall { .. });
                assert_issue!(parse_err(&s, json!("2024-02-01")), "", IssueCode::TooBig { .. });
            }

            #[test]
            fn date_past_and_future_both_sides() {
                let past = date().past();
                assert!(past.parse_value(&json!("1900-01-01")).is_ok());
                assert_issue!(
                    parse_err(&past, json!("2999-01-01")),
                    "",
                    IssueCode::NotInPast { kind: DateKind::Date }
                );
                let future = date().future();
                assert!(future.parse_value(&json!("2999-01-01")).is_ok());
                assert_issue!(
                    parse_err(&future, json!("1900-01-01")),
                    "",
                    IssueCode::NotInFuture { kind: DateKind::Date }
                );
                let err = parse_err(&date().past_msg("past only"), json!("2999-01-01"));
                assert_eq!(err.issues[0].message, "past only");
                let err = parse_err(&date().future_msg("future only"), json!("1900-01-01"));
                assert_eq!(err.issues[0].message, "future only");
            }

            #[test]
            fn datetime_accepts_only_rfc3339() {
                for ok in [
                    "2024-06-15T12:00:00Z",
                    "2024-06-15T12:00:00+03:00",
                    "2024-06-15T12:00:00.123456789-05:30",
                    "2024-06-15t12:00:00z",
                    "2024-06-15T12:00:00",
                    "2024-06-15T12:00:00.5",
                ] {
                    assert!(datetime().parse_value(&json!(ok)).is_ok(), "{ok}");
                }
                for bad in [
                    "", "not-a-date", "2024-06-15", "2024-01-01T", "2024-06-15T12:00",
                    "2024-06-15T12:00Z", "2024-06-15 12:00:00Z", "2024-6-15T12:00:00Z",
                    "2024-06-15T12:00:00+0300", "2024-06-15T12:00:00Zjunk",
                    "2024-06-15T12:00:00.Z", "2024-06-15T12:00:00.1234567890Z",
                    "2024-06-15T24:00:00Z", "2024-02-31T12:00:00Z", "2024-06-15T12:00:00[UTC]",
                    "2024-06-15T1é:00:00Z",
                ] {
                    assert_issue!(
                        parse_err(&datetime(), json!(bad)),
                        "",
                        IssueCode::InvalidDate { kind: DateKind::DateTime }
                    );
                }
                let err = parse_err(&datetime(), json!(123));
                assert!(err.issues[0].message.contains("Expected datetime string"), "{err}");
                let err = parse_err(&datetime().type_error("Datetime required!"), json!(null));
                assert_eq!(err.issues[0].message, "Datetime required!");
            }

            /// Offsets are normalized to UTC; naive input is UTC unless told otherwise.
            #[test]
            fn datetime_offsets_normalize_to_utc() {
                assert!(dt("2024-06-15T12:30:00+03:00") == dt("2024-06-15T09:30:00Z"));
                assert!(dt("2024-06-15T12:30:00.123Z") != dt("2024-06-15T12:30:00Z"));
                assert!(dt("2024-06-15T12:00:00") == dt("2024-06-15T12:00:00Z"));
                let msk = datetime().naive_timezone_offset(3 * 3600);
                assert!(msk.parse_value(&json!("2024-06-15T12:00:00")).unwrap() == dt("2024-06-15T09:00:00Z"));
                // An explicit offset wins over the naive one.
                assert!(msk.parse_value(&json!("2024-06-15T12:00:00Z")).unwrap() == dt("2024-06-15T12:00:00Z"));
            }

            #[test]
            fn datetime_naive_input_policy() {
                for strict in [datetime().with_timezone_only(), datetime().naive_allowed(false)] {
                    assert!(strict.parse_value(&json!("2024-06-15T12:30:00Z")).is_ok());
                    assert_issue!(
                        parse_err(&strict, json!("2024-06-15T12:30:00")),
                        "",
                        IssueCode::InvalidDate { kind: DateKind::DateTime }
                    );
                }
                let relaxed = datetime().with_timezone_only().naive_allowed(true);
                assert!(relaxed.parse_value(&json!("2024-06-15T12:30:00")).is_ok());
            }

            #[test]
            fn timezone_offset_only() {
                let s = datetime().timezone_offset_only(3 * 3600);
                assert!(s.parse_value(&json!("2024-06-15T12:30:00+03:00")).is_ok());
                for bad in ["2024-06-15T12:30:00+02:00", "2024-06-15T12:30:00Z", "2024-06-15T12:30:00"] {
                    let err = parse_err(&s, json!(bad));
                    assert_issue!(
                        &err,
                        "",
                        IssueCode::InvalidTimezoneOffset { expected } if expected == "+03:00"
                    );
                    assert!(err.issues[0].message.contains("+03:00"), "{err}");
                }
                let s = datetime().timezone_offset_only_msg(-5 * 3600, "EST only");
                let err = parse_err(&s, json!("2024-06-15T12:30:00Z"));
                assert_eq!(err.issues[0].message, "EST only");
                assert_issue!(&err, "", IssueCode::InvalidTimezoneOffset { expected } if expected == "-05:00");
            }

            #[test]
            fn datetime_bounds_and_relative_checks() {
                let s = datetime()
                    .min("2024-01-01T00:00:00Z")
                    .max("2024-12-31T23:59:59Z");
                assert!(s.parse_value(&json!("2024-06-15T12:00:00Z")).is_ok());
                assert!(s.parse_value(&json!("2024-01-01T03:00:00+03:00")).is_ok());
                assert_issue!(
                    parse_err(&s, json!("2023-12-31T23:59:59Z")),
                    "",
                    IssueCode::TooSmall { origin: SizeOrigin::DateTime, .. }
                );
                // 2024-01-01T02:59:59+03:00 is 2023-12-31T23:59:59Z.
                assert_issue!(
                    parse_err(&s, json!("2024-01-01T02:59:59+03:00")),
                    "",
                    IssueCode::TooSmall { .. }
                );
                let err = parse_err(&s, json!("2025-01-01T00:00:00Z"));
                assert_issue!(&err, "", IssueCode::TooBig { origin: SizeOrigin::DateTime, .. });
                assert!(err.issues[0].message.contains("2024-12-31"), "{err}");

                let s = datetime()
                    .min_datetime(dt("2024-01-01T00:00:00Z"))
                    .max_datetime(dt("2024-01-02T00:00:00Z"));
                assert!(s.parse_value(&json!("2024-01-01T12:00:00Z")).is_ok());
                assert_issue!(parse_err(&s, json!("2024-01-03T00:00:00Z")), "", IssueCode::TooBig { .. });

                let past = datetime().past_msg("in the past");
                assert!(past.parse_value(&json!("2000-01-01T00:00:00Z")).is_ok());
                let err = parse_err(&past, json!("2999-01-01T00:00:00Z"));
                assert_issue!(&err, "", IssueCode::NotInPast { kind: DateKind::DateTime });
                assert_eq!(err.issues[0].message, "in the past");
                let future = datetime().future();
                assert!(future.parse_value(&json!("2999-01-01T00:00:00Z")).is_ok());
                assert_issue!(
                    parse_err(&future, json!("2000-01-01T00:00:00Z")),
                    "",
                    IssueCode::NotInFuture { kind: DateKind::DateTime }
                );
                assert_eq!(
                    parse_err(&datetime().future_msg("later"), json!("2000-01-01T00:00:00Z")).issues[0].message,
                    "later"
                );
            }

            vld::schema! {
                #[derive(Debug)]
                pub struct Event {
                    pub title: String => vld::string().min(1),
                    pub date: $date_ty => vld::$backend::date().min("2020-01-01"),
                }
            }

            #[test]
            fn in_schema_macro_and_modifiers() {
                let e = Event::parse(r#"{"title": "Conf", "date": "2024-09-15"}"#).unwrap();
                assert_eq!(e.date.to_string(), "2024-09-15");
                let err = Event::parse(r#"{"title": "", "date": "2019-01-01"}"#).unwrap_err();
                assert_eq!(paths(&err), [".title", ".date"]);

                assert_eq!(date().optional().parse("null").unwrap(), None);
                assert!(date().optional().parse(r#""2024-01-01""#).unwrap().is_some());
                let fallback = d("2000-01-01");
                assert_eq!(
                    date().with_default(fallback).parse("null").unwrap().to_string(),
                    "2000-01-01"
                );
            }

            #[cfg(feature = "openapi")]
            #[test]
            fn json_schema_and_default() {
                use vld::openapi::JsonSchema;
                assert_eq!(date().json_schema(), json!({"type": "string", "format": "date"}));
                assert_eq!(
                    datetime().with_timezone_only().json_schema(),
                    json!({"type": "string", "format": "date-time"})
                );
                let js = date().with_default(d("2024-01-02")).json_schema();
                assert_eq!(js["default"], "2024-01-02");
            }

            /// The emitted default must be RFC 3339, i.e. parseable by `datetime()`.
            #[cfg(feature = "openapi")]
            #[test]
            $(#[$dt_default])?
            fn datetime_default_is_rfc3339() {
                use vld::openapi::JsonSchema;
                let js = datetime()
                    .with_timezone_only()
                    .with_default(dt("2024-01-02T03:04:05Z"))
                    .json_schema();
                assert_eq!(js["format"], "date-time");
                let default = js["default"].clone();
                assert!(
                    datetime().parse_value(&default).is_ok_and(|v| v == dt("2024-01-02T03:04:05Z")),
                    "default {default} does not round-trip"
                );
            }
        }
    };
}

#[cfg(feature = "chrono")]
backend_suite!(chrono, ::chrono::NaiveDate);
#[cfg(feature = "jiff")]
backend_suite!(jiff, ::jiff::civil::Date);
#[cfg(feature = "time")]
backend_suite!(time, ::time::Date,);

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

// ---- time of day ----

use crate::common::code;
use serde_json::json;
use vld::error::{DateKind, IssueCode, SizeOrigin};
use vld::prelude::*;

fn check_time_backend<B: vld::primitives::date::DateBackend>()
where
    B::Time: std::fmt::Debug,
{
    let t = vld::primitives::date::ZTime::<B>::new;
    assert!(t().parse(r#""00:00:00""#).is_ok());
    assert!(t().parse(r#""23:59:59""#).is_ok());
    assert!(t().parse(r#""12:30:45.123456789""#).is_ok());
    for bad in [
        "24:00:00",
        "12:60:00",
        "12:00:60",
        "12:00",
        "12:00:00Z",
        "12:00:00+01:00",
        "1:00:00",
        "12:00:00.",
        "12:00:00.1234567890",
        "12:00:00 ",
        "",
        "é",
    ] {
        let v = serde_json::Value::String(bad.into());
        assert_eq!(
            t().parse_value(&v).unwrap_err().issues[0].code,
            IssueCode::InvalidDate {
                kind: DateKind::Time
            },
            "{bad}"
        );
    }
    assert_eq!(code(t().parse("1200")).key(), "invalid_type");

    let s = t().min("08:00:00").max("17:30:00");
    assert!(s.parse(r#""08:00:00""#).is_ok());
    assert!(s.parse(r#""17:30:00""#).is_ok());
    let err = s.parse(r#""07:59:59.999""#).unwrap_err();
    assert_eq!(
        err.issues[0].code,
        IssueCode::TooSmall {
            origin: SizeOrigin::Time,
            minimum: json!("08:00:00"),
            inclusive: true
        }
    );
    assert_eq!(err.issues[0].message, "Time must be at or after 08:00:00");
    let err = s.parse(r#""17:30:00.5""#).unwrap_err();
    assert_eq!(err.issues[0].message, "Time must be at or before 17:30:00");

    let parsed = t().parse(r#""10:15:00""#).unwrap();
    let bounded = t().min_time(parsed.clone()).max_time(parsed);
    assert!(bounded.parse(r#""10:15:00""#).is_ok());
    assert!(bounded.parse(r#""10:15:01""#).is_err());
}

#[test]
#[should_panic(expected = "Invalid time literal")]
#[cfg(feature = "chrono")]
fn time_bad_literal_panics() {
    let _ = vld::chrono::time().min("25:00:00");
}

#[cfg(feature = "chrono")]
#[test]
fn chrono_time() {
    check_time_backend::<vld::chrono::Chrono>();
    let t = vld::chrono::time().parse(r#""09:05:01.5""#).unwrap();
    assert_eq!(
        t,
        ::chrono::NaiveTime::from_hms_milli_opt(9, 5, 1, 500).unwrap()
    );
}

#[cfg(feature = "jiff")]
#[test]
fn jiff_time() {
    check_time_backend::<vld::jiff::Jiff>();
    let t = vld::jiff::time().parse(r#""09:05:01""#).unwrap();
    assert_eq!(t, ::jiff::civil::time(9, 5, 1, 0));
}

#[cfg(feature = "time")]
#[test]
fn time_crate_time() {
    check_time_backend::<vld::time::Time>();
    let t = vld::time::time().parse(r#""09:05:01.25""#).unwrap();
    assert_eq!(t, ::time::Time::from_hms_milli(9, 5, 1, 250).unwrap());
    // Bound display is zero-padded HH:MM:SS for the `time` crate too.
    let s = vld::time::time().min_time(::time::Time::from_hms(8, 0, 0).unwrap());
    let err = s.parse(r#""07:00:00""#).unwrap_err();
    assert!(err.issues[0].message.ends_with("08:00:00.0"), "{err}");
}

#[cfg(any(
    all(feature = "chrono", not(feature = "jiff"), not(feature = "time")),
    all(feature = "jiff", not(feature = "chrono"), not(feature = "time")),
    all(feature = "time", not(feature = "chrono"), not(feature = "jiff"))
))]
#[test]
fn time_of_day_shortcut() {
    let s: ZTime = vld::time_of_day().max("12:00:00");
    assert!(s.parse(r#""11:59:59""#).is_ok());
    assert!(s.parse(r#""12:00:01""#).is_err());
}

#[cfg(all(feature = "time", feature = "openapi"))]
#[test]
fn time_datetime_default_is_rfc3339() {
    use vld::json_schema::JsonSchema;
    let dt = vld::time::datetime()
        .parse_value(&json!("2024-01-02T03:04:05Z"))
        .unwrap();
    let schema = vld::time::datetime().with_default(dt).json_schema();
    let default = schema["default"].clone();
    assert!(
        vld::time::datetime().parse_value(&default).is_ok(),
        "{default}"
    );
}
