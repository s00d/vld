use serde_json::json;
use vld::prelude::*;

#[test]
fn optional() {
    let s = vld::string().optional();
    assert_eq!(s.parse_value(&json!(null)).unwrap(), None);
    assert_eq!(
        s.parse_value(&json!("hello")).unwrap(),
        Some("hello".to_string())
    );
}

#[test]
fn default_value() {
    let s = vld::string().with_default("world".to_string());
    assert_eq!(s.parse_value(&json!(null)).unwrap(), "world");
    assert_eq!(s.parse_value(&json!("hello")).unwrap(), "hello");
    assert!(s.parse_value(&json!(42)).is_err());
}

#[test]
fn nullish() {
    let s = vld::string().nullish();
    assert_eq!(s.parse_value(&json!(null)).unwrap(), None);
    assert_eq!(s.parse_value(&json!("hi")).unwrap(), Some("hi".to_string()));
}

#[test]
fn catch_fallback() {
    let s = vld::string().min(6).catch("fallback".to_string());
    assert_eq!(s.parse_value(&json!("short")).unwrap(), "fallback");
    assert_eq!(s.parse_value(&json!("long enough")).unwrap(), "long enough");
    assert_eq!(s.parse_value(&json!(42)).unwrap(), "fallback");
}

#[test]
fn chained_modifiers() {
    let schema = vld::string()
        .optional()
        .transform(|opt| opt.unwrap_or_else(|| "default".to_string()));
    assert_eq!(schema.parse_value(&json!(null)).unwrap(), "default");
    assert_eq!(schema.parse_value(&json!("hello")).unwrap(), "hello");
}
