//! Same accept/reject table for every date backend. Run with each of
//! `--features chrono`, `--features jiff`, `--features time`.
#![cfg(any(feature = "chrono", feature = "jiff", feature = "time"))]

use vld::prelude::*;

#[test]
fn date_accepts_only_strict_iso() {
    let s = vld::date();
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
    let s = vld::datetime();
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
    let s = vld::datetime().timezone_offset_only(3 * 3600);
    assert!(s.parse(r#""2024-06-15T12:30:00+03:00""#).is_ok());
    let err = s.parse(r#""2024-06-15T12:30:00""#).unwrap_err();
    assert_eq!(err.issues[0].code.key(), "invalid_timezone_offset");
    assert!(err.issues[0].message.contains("+03:00"));
}

#[test]
fn date_bounds_report_real_bound() {
    let err = vld::date()
        .min("2024-01-01")
        .parse(r#""2023-01-01""#)
        .unwrap_err();
    assert_eq!(err.issues[0].code.key(), "date_too_small");
    let tr = vld::i18n::translate_error(&err, &vld::i18n::english());
    assert!(tr.issues[0].message.contains("2024-01-01"));

    let err = vld::datetime()
        .max("2024-01-01T00:00:00Z")
        .parse(r#""2025-01-01T00:00:00Z""#)
        .unwrap_err();
    assert_eq!(err.issues[0].code.key(), "date_too_big");
    assert!(err.issues[0].message.contains("2024-01-01"));
}
