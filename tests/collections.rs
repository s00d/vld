use serde_json::json;
use vld::prelude::*;
use std::f64::consts::PI;

#[test]
fn array_basic() {
    let a = vld::array(vld::string()).min_len(1).max_len(3);
    assert!(a.parse_value(&json!([])).is_err());
    assert_eq!(
        a.parse_value(&json!(["a", "b"])).unwrap(),
        vec!["a".to_string(), "b".to_string()]
    );
    assert!(a.parse_value(&json!(["a", "b", "c", "d"])).is_err());
}

#[test]
fn array_element_validation() {
    let a = vld::array(vld::string().min(2));
    let result = a.parse_value(&json!(["ok", "x", "fine", "y"]));
    assert!(result.is_err());
    assert!(result.unwrap_err().issues.len() >= 2);
}

#[test]
fn array_contains_unique_and_counts() {
    let a = vld::array(vld::number().int())
        .contains(json!(2))
        .min_contains(2)
        .max_contains(3)
        .unique();
    assert!(a.parse_value(&json!([1, 2, 2])).is_err()); // unique fails
    assert!(a.parse_value(&json!([1, 2])).is_err()); // min_contains fails
    assert!(a.parse_value(&json!([1, 2, 2, 2, 2])).is_err()); // max_contains + unique fail
}

#[test]
fn tuple2() {
    let schema = (vld::string(), vld::number().int());
    assert_eq!(
        schema.parse_value(&json!(["hello", 42])).unwrap(),
        ("hello".to_string(), 42)
    );
    assert!(schema.parse_value(&json!(["hello"])).is_err());
    assert!(schema.parse_value(&json!([42, "hello"])).is_err());
}

#[test]
fn tuple3() {
    let schema = (vld::string(), vld::number(), vld::boolean());
    let result = schema.parse_value(&json!(["hi", PI, true])).unwrap();
    assert_eq!(result, ("hi".to_string(), PI, true));
}

#[test]
fn record_basic() {
    let r = vld::record(vld::number().int().positive());
    let result = r.parse_value(&json!({"a": 1, "b": 2, "c": 3})).unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(result.get("b"), Some(&2));
}

#[test]
fn record_validation() {
    let r = vld::record(vld::number().positive());
    assert!(r.parse_value(&json!({"a": 1, "b": -1})).is_err());
}

#[test]
fn record_min_max_keys() {
    let r = vld::record(vld::string()).min_keys(1).max_keys(2);
    assert!(r.parse_value(&json!({})).is_err());
    assert!(r.parse_value(&json!({"a": "1"})).is_ok());
    assert!(r
        .parse_value(&json!({"a": "1", "b": "2", "c": "3"}))
        .is_err());
}
