//! Stories: settings already loaded into a JSON value (a custom source).

use crate::common::*;

use serde_json::json;
use std::error::Error;
use vld_config::{VldConfigError, from_value, from_value_coerced};

/// Given settings loaded by the app itself into JSON,
/// when it validates them with `from_value`,
/// then valid ones come back typed (optional key absent or present), broken
/// ones list every bad key, and a number where text is expected is an error
/// (`from_value` is strict).
#[test]
fn strict_validation_of_a_loaded_value() {
    let ok = json!({
        "host": "0.0.0.0", "port": 8080, "debug": true, "tags": ["a"],
        "db": {"url": "https://db.example.com", "pool_size": 10}
    });
    let s: Settings = from_value(&ok).unwrap();
    assert_eq!((s.host.as_str(), s.port, s.debug), ("0.0.0.0", 8080, true));
    assert_eq!((s.db.pool_size, s.db.name), (10, None));

    let mut named = ok.clone();
    named["db"]["name"] = json!("mydb");
    let s: Settings = from_value(&named).unwrap();
    assert_eq!(s.db.name.as_deref(), Some("mydb"));

    assert_eq!(
        issues(from_value::<Flat>(
            &json!({"host": "", "port": 0, "debug": true, "tags": []})
        )),
        pairs(&[(".host", "too_small"), (".port", "too_small")])
    );
    assert_eq!(
        issues(from_value::<Flat>(&json!({"host": "localhost"}))),
        pairs(&[
            (".port", "missing_field"),
            (".debug", "missing_field"),
            (".tags", "missing_field")
        ])
    );
    let mut bad_url = ok.clone();
    bad_url["db"]["url"] = json!("not-a-url");
    assert_eq!(
        issues(from_value::<Settings>(&bad_url)),
        pairs(&[(".db.url", "invalid_string")])
    );

    let typed = json!({"host": 12345, "port": 1, "debug": false, "tags": true});
    assert_eq!(
        issues(from_value::<Flat>(&typed)),
        pairs(&[(".host", "invalid_type"), (".tags", "invalid_type")])
    );
}

/// Given the same typed value (numbers / booleans where text is expected, a
/// single value where a list is expected, also inside a section),
/// when the app validates it with `from_value_coerced`,
/// then it is coerced like the backends do; values that cannot fit still
/// fail.
#[test]
fn coerced_validation_matches_the_backends() {
    let typed = json!({
        "host": 12345, "port": 1, "debug": false, "tags": true,
        "db": {"url": "https://x.io", "pool_size": 3, "name": 7}
    });
    let s: Settings = from_value_coerced(&typed).unwrap();
    assert_eq!(
        (s.host.as_str(), s.tags.as_slice()),
        ("12345", ["true".to_string()].as_slice())
    );
    assert_eq!(s.db.name.as_deref(), Some("7"));

    assert_eq!(
        issues(from_value_coerced::<Flat>(
            &json!({"host": "h", "port": "80", "debug": 1, "tags": []})
        )),
        pairs(&[(".port", "invalid_type"), (".debug", "invalid_type")])
    );
}

/// Given the two kinds of failure,
/// when the app prints them and walks `source()`,
/// then a source error reads `config error: <msg>` without a source, a
/// validation error reads `validation error: <issues>` with the `VldError`;
/// a `VldError` converts into the validation kind.
#[test]
fn error_kinds() {
    let err = VldConfigError::Source("test error".into());
    assert_eq!(err.to_string(), "config error: test error");
    assert!(err.source().is_none());

    let vld_err = from_value::<Flat>(&json!({})).unwrap_err();
    let VldConfigError::Validation(inner) = &vld_err else {
        panic!("{vld_err:?}")
    };
    assert_eq!(vld_err.to_string(), format!("validation error: {inner}"));
    let source = vld_err.source().expect("the VldError");
    assert_eq!(source.to_string(), inner.to_string());

    let converted: VldConfigError = inner.clone().into();
    assert!(matches!(converted, VldConfigError::Validation(e) if e.issues.len() == 4));
}
