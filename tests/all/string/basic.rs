//! `vld::string()`: type, length, content checks and transforms.

use crate::common::{codes, parse_err};
use serde_json::json;
use vld::error::{IssueCode, SizeOrigin, StringValidation};
use vld::prelude::*;

#[test]
fn accepts_strings_only() {
    let s = vld::string();
    assert_eq!(s.parse_value(&json!("hello")).unwrap(), "hello");
    assert_eq!(s.parse_value(&json!("")).unwrap(), "");
    for bad in [json!(42), json!(null), json!(true), json!([]), json!({})] {
        let err = parse_err(&s, bad.clone());
        assert_issue!(&err, "", IssueCode::InvalidType { expected, .. } if expected == "string");
        assert_eq!(err.issues[0].received, Some(bad));
    }
}

/// Lengths count chars, not bytes: `é` is one char, two bytes.
#[test]
fn min_max_len_count_chars() {
    let s = vld::string().min(2).max(3);
    let cases: &[(&str, Option<&str>)] = &[
        ("", Some("too_small")),
        ("a", Some("too_small")),
        ("é", Some("too_small")),
        ("ab", None),
        ("éé", None),
        ("日本語", None),
        ("abc", None),
        ("abcd", Some("too_big")),
        ("éééé", Some("too_big")),
    ];
    for (input, want) in cases {
        let got = s.parse_value(&json!(input));
        match want {
            None => assert_eq!(got.unwrap(), *input),
            Some(code) => assert_eq!(codes(&got.unwrap_err()), [*code], "{input:?}"),
        }
    }
    assert_issue!(
        parse_err(&s, json!("a")),
        "",
        IssueCode::TooSmall { origin: SizeOrigin::String, minimum, inclusive: true } if minimum == &json!(2)
    );
    assert_issue!(
        parse_err(&s, json!("abcd")),
        "",
        IssueCode::TooBig { origin: SizeOrigin::String, maximum, inclusive: true } if maximum == &json!(3)
    );
}

#[test]
fn len_and_non_empty() {
    let s = vld::string().len(3);
    assert!(s.parse_value(&json!("abc")).is_ok());
    assert!(s.parse_value(&json!("été")).is_ok());
    for bad in ["ab", "abcd", ""] {
        assert_issue!(
            parse_err(&s, json!(bad)),
            "",
            IssueCode::InvalidLength {
                origin: SizeOrigin::String,
                expected: 3
            }
        );
    }

    let s = vld::string().non_empty();
    assert_eq!(s.parse_value(&json!(" ")).unwrap(), " ");
    assert_issue!(parse_err(&s, json!("")), "", IssueCode::TooSmall { .. });
}

#[test]
fn starts_ends_contains() {
    let s = vld::string()
        .starts_with("foo")
        .ends_with("bar")
        .contains("oba");
    assert_eq!(s.parse_value(&json!("foobar")).unwrap(), "foobar");

    let err = parse_err(&s, json!("xyz"));
    assert_eq!(err.issues.len(), 3);
    assert_issue!(&err, "", IssueCode::InvalidString { validation: StringValidation::StartsWith { prefix } } if prefix == "foo");
    assert_issue!(&err, "", IssueCode::InvalidString { validation: StringValidation::EndsWith { suffix } } if suffix == "bar");
    assert_issue!(&err, "", IssueCode::InvalidString { validation: StringValidation::Contains { substring } } if substring == "oba");
}

#[test]
fn all_failing_checks_are_reported() {
    let err = parse_err(&vld::string().min(5).email().url(), json!("ab"));
    assert_eq!(
        codes(&err),
        ["too_small", "invalid_string", "invalid_string"]
    );
}

#[test]
fn transforms_run_before_checks() {
    let s = vld::string().trim().to_lowercase();
    assert_eq!(s.parse_value(&json!("  Hello  ")).unwrap(), "hello");
    let s = vld::string().to_uppercase();
    assert_eq!(s.parse_value(&json!("straße")).unwrap(), "STRASSE");

    let s = vld::string().trim().min(3);
    assert_eq!(s.parse_value(&json!("  abc  ")).unwrap(), "abc");
    assert_issue!(
        parse_err(&s, json!("  ab  ")),
        "",
        IssueCode::TooSmall { .. }
    );
}

#[test]
fn coerce_stringifies_scalars_only() {
    let s = vld::string().coerce();
    assert_eq!(s.parse_value(&json!(42)).unwrap(), "42");
    assert_eq!(s.parse_value(&json!(1.5)).unwrap(), "1.5");
    assert_eq!(s.parse_value(&json!(true)).unwrap(), "true");
    assert_eq!(s.parse_value(&json!("x")).unwrap(), "x");
    for bad in [json!(null), json!([1]), json!({"a": 1})] {
        assert_issue!(parse_err(&s, bad), "", IssueCode::InvalidType { .. });
    }
}

#[cfg(feature = "regex")]
#[test]
fn regex_check() {
    let s = vld::string().regex(vld::regex_lite::Regex::new(r"^[a-z]+\d$").unwrap());
    assert!(s.parse_value(&json!("abc1")).is_ok());
    for bad in ["abc", "ABC1", "", "é1"] {
        assert_issue!(
            parse_err(&s, json!(bad)),
            "",
            IssueCode::InvalidString {
                validation: StringValidation::Regex
            }
        );
    }
}
