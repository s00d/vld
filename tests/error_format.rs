use serde_json::json;
use vld::prelude::*;

#[test]
fn prettify_error() {
    let schema = vld::object()
        .field("name", vld::string().min(5))
        .field("age", vld::number().positive());

    let err = schema
        .parse_value(&json!({"name": "ab", "age": -1}))
        .unwrap_err();
    let pretty = vld::format::prettify_error(&err);
    assert!(pretty.contains("✖"));
    assert!(pretty.contains("name"));
    assert!(pretty.contains("age"));
}

#[test]
fn flatten_error() {
    let schema = vld::object()
        .field("name", vld::string().min(5))
        .field("age", vld::number().positive());

    let err = schema
        .parse_value(&json!({"name": "ab", "age": -1}))
        .unwrap_err();
    let flat = vld::format::flatten_error(&err);
    assert!(flat.field_errors.contains_key("name"));
    assert!(flat.field_errors.contains_key("age"));
}

#[test]
fn treeify_error() {
    let schema = vld::object().field("user", vld::object().field("name", vld::string().min(5)));

    let err = schema
        .parse_value(&json!({"user": {"name": "ab"}}))
        .unwrap_err();
    let tree = vld::format::treeify_error(&err);
    assert!(tree.properties.contains_key("user"));
    assert!(tree.properties["user"].properties.contains_key("name"));
}
