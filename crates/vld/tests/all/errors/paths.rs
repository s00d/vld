//! Issue paths through nested schemas, and `received` truncation.

use crate::common::paths;
use serde_json::json;
use vld::prelude::*;

#[test]
fn nested_object_array_record_paths() {
    let s = vld::object().field(
        "users",
        vld::array(
            vld::object()
                .field("name", vld::string().min(1))
                .field("tags", vld::record(vld::number().int())),
        ),
    );
    let err = s
        .parse_value(&json!({"users": [
            {"name": "ok", "tags": {}},
            {"name": "", "tags": {"a": 1, "b": "x"}}
        ]}))
        .unwrap_err();
    assert_eq!(paths(&err), [".users[1].name", ".users[1].tags.b"]);
}

/// Long received values are cut at 100 chars (not bytes) and never split a char.
#[test]
fn received_value_truncation_is_utf8_safe() {
    let s = "é".repeat(51);
    let err = vld::string().max(1).parse_value(&json!(s)).unwrap_err();
    assert_eq!(err.issues[0].received, Some(json!(s)));

    let long = "é".repeat(150);
    let err = vld::string().max(1).parse_value(&json!(long)).unwrap_err();
    let got = err.issues[0].received.as_ref().unwrap().as_str().unwrap();
    assert_eq!(got.chars().count(), 100);
    assert!(got.ends_with("..."));
    assert!(got.starts_with("éééé"));

    let mixed = "a😀".repeat(80);
    let err = vld::string().max(1).parse_value(&json!(mixed)).unwrap_err();
    let got = err.issues[0].received.as_ref().unwrap().as_str().unwrap();
    assert_eq!(got.chars().count(), 100);
}
