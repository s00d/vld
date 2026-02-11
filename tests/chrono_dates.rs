#![cfg(feature = "chrono")]

use vld::prelude::*;

// ---------------------------------------------------------------------------
// ZDate
// ---------------------------------------------------------------------------

#[test]
fn date_parse_valid() {
    let schema = vld::date();
    let d = schema.parse(r#""2024-06-15""#).unwrap();
    assert_eq!(d.to_string(), "2024-06-15");
}

#[test]
fn date_parse_invalid_format() {
    let schema = vld::date();
    let err = schema.parse(r#""06/15/2024""#).unwrap_err();
    assert!(err.issues[0].message.contains("Invalid date format"));
}

#[test]
fn date_parse_not_string() {
    let schema = vld::date();
    let err = schema.parse("42").unwrap_err();
    assert!(err.issues[0].message.contains("Expected date string"));
}

#[test]
fn date_type_error() {
    let schema = vld::date().type_error("Date required!");
    let err = schema.parse("true").unwrap_err();
    assert_eq!(err.issues[0].message, "Date required!");
}

#[test]
fn date_min_constraint() {
    let schema = vld::date().min("2024-01-01");

    let ok = schema.parse(r#""2024-06-15""#);
    assert!(ok.is_ok());

    let err = schema.parse(r#""2023-12-31""#).unwrap_err();
    assert!(err.issues[0].message.contains("on or after"));
}

#[test]
fn date_max_constraint() {
    let schema = vld::date().max("2025-12-31");

    let ok = schema.parse(r#""2025-06-15""#);
    assert!(ok.is_ok());

    let err = schema.parse(r#""2026-01-01""#).unwrap_err();
    assert!(err.issues[0].message.contains("on or before"));
}

#[test]
fn date_min_max_combined() {
    let schema = vld::date().min("2024-01-01").max("2024-12-31");

    assert!(schema.parse(r#""2024-07-04""#).is_ok());
    assert!(schema.parse(r#""2023-12-31""#).is_err());
    assert!(schema.parse(r#""2025-01-01""#).is_err());
}

// ---------------------------------------------------------------------------
// ZDateTime
// ---------------------------------------------------------------------------

#[test]
fn datetime_parse_rfc3339() {
    let schema = vld::datetime();
    let dt = schema.parse(r#""2024-06-15T12:30:00Z""#).unwrap();
    assert_eq!(dt.format("%Y-%m-%d").to_string(), "2024-06-15");
}

#[test]
fn datetime_parse_with_offset() {
    let schema = vld::datetime();
    let dt = schema.parse(r#""2024-06-15T12:30:00+03:00""#).unwrap();
    // Converted to UTC: 09:30
    assert_eq!(dt.format("%H:%M").to_string(), "09:30");
}

#[test]
fn datetime_parse_with_millis() {
    let schema = vld::datetime();
    let dt = schema.parse(r#""2024-06-15T12:30:00.123Z""#).unwrap();
    assert_eq!(dt.format("%Y-%m-%d").to_string(), "2024-06-15");
}

#[test]
fn datetime_parse_invalid_format() {
    let schema = vld::datetime();
    let err = schema.parse(r#""not-a-date""#).unwrap_err();
    assert!(err.issues[0].message.contains("Invalid datetime format"));
}

#[test]
fn datetime_parse_not_string() {
    let schema = vld::datetime();
    let err = schema.parse("123").unwrap_err();
    assert!(err.issues[0].message.contains("Expected datetime string"));
}

#[test]
fn datetime_type_error() {
    let schema = vld::datetime().type_error("Datetime required!");
    let err = schema.parse("null").unwrap_err();
    assert_eq!(err.issues[0].message, "Datetime required!");
}

// ---------------------------------------------------------------------------
// In schema! macro
// ---------------------------------------------------------------------------

vld::schema! {
    #[derive(Debug)]
    pub struct Event {
        pub title: String => vld::string().min(1),
        pub date: chrono::NaiveDate => vld::date().min("2020-01-01"),
    }
}

#[test]
fn schema_macro_with_date() {
    let json = r#"{"title": "Conference", "date": "2024-09-15"}"#;
    let event = Event::parse(json).unwrap();
    assert_eq!(event.title, "Conference");
    assert_eq!(event.date.to_string(), "2024-09-15");
}

#[test]
fn schema_macro_with_date_error() {
    let json = r#"{"title": "Old Event", "date": "2019-01-01"}"#;
    let err = Event::parse(json).unwrap_err();
    assert!(err.issues.iter().any(|i| {
        let path: String = i.path.iter().map(|p| p.to_string()).collect();
        path.contains("date")
    }));
}

// ---------------------------------------------------------------------------
// With modifiers
// ---------------------------------------------------------------------------

#[test]
fn date_optional() {
    let schema = vld::date().optional();
    assert_eq!(schema.parse("null").unwrap(), None);
    assert!(schema.parse(r#""2024-01-01""#).unwrap().is_some());
}

#[test]
fn date_with_default() {
    let fallback = chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();
    let schema = vld::date().with_default(fallback);
    let d = schema.parse("null").unwrap();
    assert_eq!(d.to_string(), "2000-01-01");
}

// ---------------------------------------------------------------------------
// JsonSchema
// ---------------------------------------------------------------------------

#[cfg(feature = "openapi")]
#[test]
fn date_json_schema() {
    use vld::json_schema::JsonSchema;

    let js = vld::date().json_schema();
    assert_eq!(js["type"], "string");
    assert_eq!(js["format"], "date");
}

#[cfg(feature = "openapi")]
#[test]
fn datetime_json_schema() {
    use vld::json_schema::JsonSchema;

    let js = vld::datetime().json_schema();
    assert_eq!(js["type"], "string");
    assert_eq!(js["format"], "date-time");
}
