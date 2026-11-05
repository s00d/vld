//! Inputs (`VldInput`) and validating existing Rust values (`validate` / `is_valid`).

use crate::common::{codes, paths, tempdir};
use serde_json::json;
use vld::error::IssueCode;
use vld::prelude::*;
use vld::schema::VldInput;

#[test]
fn every_input_kind_parses() {
    let s = vld::string().min(2);
    assert_eq!(s.parse(r#""hello""#).unwrap(), "hello");
    assert_eq!(s.parse(&String::from(r#""world""#)).unwrap(), "world");
    assert_eq!(vld::number().parse(b"42".as_ref()).unwrap(), 42.0);
    assert_eq!(s.parse(&json!("value")).unwrap(), "value");

    let dir = tempdir();
    let path = dir.path().join("in.json");
    std::fs::write(&path, r#""from file""#).unwrap();
    assert_eq!(s.parse(path.as_path()).unwrap(), "from file");
}

#[test]
fn malformed_input_is_a_parse_error() {
    for bad in ["{", "", "'x'", "\"unterminated"] {
        let err = vld::string().parse(bad).unwrap_err();
        assert_eq!(err.issues.len(), 1, "{bad:?}");
        assert_eq!(err.issues[0].code, IssueCode::ParseError, "{bad:?}");
    }
    assert_eq!(
        codes(&vld::string().parse(b"\xff".as_ref()).unwrap_err()),
        ["parse_error"]
    );
    let missing = std::path::Path::new("definitely/not/here.json");
    assert!(missing.to_json_value().is_err());
    assert_eq!(
        codes(&vld::string().parse(missing).unwrap_err()),
        ["io_error"]
    );
}

#[test]
fn validate_rust_values_through_serialize() {
    let s = vld::array(vld::number().int().positive()).min_len(1);
    assert_eq!(s.validate(&vec![1, 2, 3]).unwrap(), vec![1, 2, 3]);
    assert_eq!(
        paths(&s.validate(&vec![-1, 1, 0]).unwrap_err()),
        ["[0]", "[2]"]
    );
    assert!(!s.is_valid(&Vec::<i64>::new()));

    assert!(vld::string().email().is_valid(&"user@example.com"));
    assert!(!vld::string().email().is_valid(&"not-an-email"));
    assert!(vld::number().max(100.0).is_valid(&50));
    assert!(!vld::number().max(100.0).is_valid(&200));

    let r = vld::record(vld::number().int().positive());
    let map = std::collections::HashMap::from([("a", 1), ("b", -2)]);
    assert_eq!(paths(&r.validate(&map).unwrap_err()), [".b"]);
}
