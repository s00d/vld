use serde_json::json;
use vld::prelude::*;

#[test]
fn dynamic_object() {
    let schema = vld::object()
        .field("name", vld::string().min(1))
        .field("age", vld::number().int().min(0));
    assert!(schema
        .parse_value(&json!({"name": "Alex", "age": 25}))
        .is_ok());
}

#[test]
fn dynamic_object_strict() {
    let schema = vld::object().field("name", vld::string()).strict();
    assert!(schema
        .parse_value(&json!({"name": "Alex", "extra": "field"}))
        .is_err());
}

#[test]
fn dynamic_object_min_max_keys() {
    let schema = vld::object()
        .field("name", vld::string())
        .min_keys(1)
        .max_keys(2)
        .passthrough();
    assert!(schema.parse_value(&json!({})).is_err());
    assert!(schema.parse_value(&json!({"name":"Alex"})).is_ok());
    assert!(schema
        .parse_value(&json!({"name":"Alex","extra":"1","extra2":"2"}))
        .is_err());
}
