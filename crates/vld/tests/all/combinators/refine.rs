//! `refine`, `super_refine` (+ `IssueBuilder`), `custom`.

use crate::common::{codes, parse_err};
use serde_json::json;
use vld::error::IssueCode;
use vld::prelude::*;

fn custom(code: &str) -> IssueCode {
    IssueCode::Custom { code: code.into() }
}

#[test]
fn refine_runs_after_inner_succeeds() {
    let even = vld::number().int().refine(|n| n % 2 == 0, "Must be even");
    assert_eq!(even.parse_value(&json!(4)).unwrap(), 4);
    let err = parse_err(&even, json!(3));
    assert_eq!(err.issues.len(), 1);
    assert_eq!(err.issues[0].code, custom("custom"));
    assert_eq!(err.issues[0].message, "Must be even");
    // The inner error short-circuits the predicate.
    assert_eq!(codes(&parse_err(&even, json!("x"))), ["invalid_type"]);
}

#[test]
fn super_refine_reports_every_pushed_issue() {
    let s = vld::string().super_refine(|s, errors| {
        if s.len() < 3 {
            errors.push(custom("too_short"), "Too short");
        }
        if !s.contains('@') {
            errors.push(custom("no_at"), "Missing @");
        }
    });
    let err = s.parse(r#""hi""#).unwrap_err();
    assert_eq!(codes(&err), ["too_short", "no_at"]);
    assert_eq!(err.issues[1].message, "Missing @");
    assert_eq!(s.parse(r#""a@bc""#).unwrap(), "a@bc");

    let noop = vld::number().int().super_refine(|_n, _errors| {});
    assert_eq!(noop.parse("42").unwrap(), 42);
}

#[test]
fn super_refine_with_issue_builder() {
    let s = vld::string().super_refine(|s, errors| {
        if !s.contains('@') {
            errors
                .issue(custom("no_at"))
                .message("Missing @ symbol")
                .path_field("contact")
                .finish();
        }
    });
    let err = s.parse(r#""ab""#).unwrap_err();
    assert_issue!(&err, ".contact", IssueCode::Custom { code } if code == "no_at");
    assert_eq!(err.issues[0].message, "Missing @ symbol");
}

#[test]
fn custom_schema() {
    let even = vld::custom(|v: &serde_json::Value| {
        let n = v.as_i64().ok_or_else(|| "Expected integer".to_string())?;
        if n % 2 == 0 {
            Ok(n)
        } else {
            Err("Must be even".to_string())
        }
    });
    assert_eq!(even.parse("4").unwrap(), 4);
    let err = even.parse("5").unwrap_err();
    assert_eq!(err.issues[0].code, custom("custom"));
    assert_eq!(err.issues[0].message, "Must be even");
    assert_eq!(err.issues[0].received, Some(json!(5)));
    assert_eq!(
        even.parse(r#""x""#).unwrap_err().issues[0].message,
        "Expected integer"
    );
}
