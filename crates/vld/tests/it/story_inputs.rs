//! Stories: where the input comes from (`VldInput`: JSON text, `String`, bytes, a
//! `serde_json::Value`, a file path) and validating Rust values already in memory
//! (`validate` / `is_valid`).

use crate::common::*;
use serde_json::json;
use vld::prelude::*;
use vld::schema::VldInput;

/// Given one string schema
/// When the same document arrives as `&str`, `&String`, bytes, a `Value` or a file on disk, and
/// then as malformed text, invalid UTF-8 bytes or a path that does not exist
/// Then every well-formed source parses to the same kind of output, malformed text and bytes are
/// one root `parse_error`, and a missing file is one `io_error` (its `to_json_value` fails too).
#[test]
fn one_schema_reads_every_input_source() {
    let s = vld::string().min(2);
    assert_eq!(s.parse(r#""hello""#).unwrap(), "hello");
    assert_eq!(s.parse(&String::from(r#""world""#)).unwrap(), "world");
    assert_eq!(vld::number().parse(b"42".as_ref()).unwrap(), 42.0);
    assert_eq!(s.parse(&json!("value")).unwrap(), "value");

    let dir = tempdir();
    let path = dir.path().join("in.json");
    std::fs::write(&path, r#""from file""#).unwrap();
    assert_eq!(s.parse(path.as_path()).unwrap(), "from file");
    assert_eq!(s.parse(&path).unwrap(), "from file");

    for bad in ["{", "", "'x'", "\"unterminated"] {
        let err = s.parse(bad).unwrap_err();
        assert_eq!(
            issues(&err),
            [(String::new(), "parse_error".into())],
            "{bad:?}"
        );
    }
    assert_eq!(
        issues(&s.parse(b"\xff".as_ref()).unwrap_err()),
        [(String::new(), "parse_error".into())]
    );

    let missing = std::path::Path::new("definitely/not/here.json");
    assert!(missing.to_json_value().is_err());
    assert_eq!(
        issues(&s.parse(missing).unwrap_err()),
        [(String::new(), "io_error".into())]
    );
}

/// Given schemas for a list of ids, an email, a capped number and a record of counts
/// When Rust values (`Vec`, `&str`, integers, a `HashMap`) are checked with `validate` / `is_valid`
/// Then they are serialized and validated like JSON: the output comes back on success, and each
/// failing element or key is reported at its index / key path.
#[test]
fn rust_values_are_validated_through_serialize() {
    let ids = vld::array(vld::number().int().positive()).min_len(1);
    assert_eq!(ids.validate(&vec![1, 2, 3]).unwrap(), vec![1, 2, 3]);
    assert_eq!(
        issues(&ids.validate(&vec![-1, 1, 0]).unwrap_err()),
        [
            ("[0]".into(), "too_small".into()),
            ("[2]".into(), "too_small".into())
        ]
    );
    assert!(!ids.is_valid(&Vec::<i64>::new()));

    assert!(vld::string().email().is_valid(&"user@example.com"));
    assert!(!vld::string().email().is_valid(&"not-an-email"));
    assert!(vld::number().max(100.0).is_valid(&50));
    assert!(!vld::number().max(100.0).is_valid(&200));

    let counts = vld::record(vld::number().int().positive());
    let map = std::collections::HashMap::from([("a", 1), ("b", -2)]);
    assert_eq!(
        issues(&counts.validate(&map).unwrap_err()),
        [(".b".into(), "too_small".into())]
    );
}
