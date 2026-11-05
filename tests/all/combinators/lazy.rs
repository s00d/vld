use crate::common::{codes, paths};
use serde_json::json;
use vld::prelude::*;

#[test]
fn lazy_wraps_a_factory() {
    let s = vld::lazy(|| vld::string().min(3));
    assert_eq!(s.parse(r#""hello""#).unwrap(), "hello");
    assert_eq!(codes(&s.parse(r#""hi""#).unwrap_err()), ["too_small"]);
}

fn tree() -> vld::object::ZObject {
    vld::object()
        .field("value", vld::number().int())
        .field("children", vld::array(vld::lazy(tree)))
}

#[test]
fn recursive_schema_and_deep_error_paths() {
    let input = r#"{"value": 1, "children": [
        {"value": 2, "children": []},
        {"value": 3, "children": [{"value": 4, "children": []}]}
    ]}"#;
    let out = tree().parse(input).unwrap();
    assert_eq!(out["children"][1]["children"][0]["value"], json!(4));

    let bad =
        r#"{"value": 1, "children": [{"value": 2, "children": [{"value": "x", "children": []}]}]}"#;
    assert_eq!(
        paths(&tree().parse(bad).unwrap_err()),
        [".children[0].children[0].value"]
    );
}

#[test]
fn factory_called_once_and_cached() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    let s = vld::lazy(|| {
        CALLS.fetch_add(1, Ordering::Relaxed);
        vld::string()
    });
    let _ = s.parse(r#""a""#);
    let _ = s.parse(r#""b""#);
    assert_eq!(CALLS.load(Ordering::Relaxed), 1);
}
