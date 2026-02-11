use serde_json::json;
use vld::prelude::*;

#[test]
fn refine() {
    let even = vld::number().int().refine(|n| n % 2 == 0, "Must be even");
    assert!(even.parse_value(&json!(4)).is_ok());
    assert!(even.parse_value(&json!(3)).is_err());
}

#[test]
fn transform() {
    let len = vld::string().transform(|s| s.len());
    assert_eq!(len.parse_value(&json!("hello")).unwrap(), 5);
}

#[test]
fn union2() {
    let schema = vld::union(vld::string(), vld::number().int());

    let r1 = schema.parse_value(&json!("hello")).unwrap();
    assert!(r1.is_left());
    assert_eq!(r1.left().unwrap(), "hello");

    let r2 = schema.parse_value(&json!(42)).unwrap();
    assert!(r2.is_right());
    assert_eq!(r2.right().unwrap(), 42);

    assert!(schema.parse_value(&json!(true)).is_err());
}

#[test]
fn union3() {
    let schema = vld::union3(vld::string(), vld::number().int(), vld::boolean());
    assert!(schema.parse_value(&json!("hi")).is_ok());
    assert!(schema.parse_value(&json!(42)).is_ok());
    assert!(schema.parse_value(&json!(true)).is_ok());
    assert!(schema.parse_value(&json!(null)).is_err());
}

#[test]
fn literal_in_union() {
    let schema = vld::union(vld::literal("hello"), vld::literal(42i64));
    assert!(schema.parse_value(&json!("hello")).is_ok());
    assert!(schema.parse_value(&json!(42)).is_ok());
    assert!(schema.parse_value(&json!("world")).is_err());
}

#[test]
fn pipe() {
    let schema = vld::string()
        .transform(|s| s.len())
        .pipe(vld::number().min(3.0));
    assert!(schema.parse_value(&json!("hello")).is_ok());
    assert!(schema.parse_value(&json!("hi")).is_err());
}

#[test]
fn preprocess() {
    let schema = vld::preprocess(
        |v| match v.as_str() {
            Some(s) => serde_json::json!(s.trim()),
            None => v.clone(),
        },
        vld::string().min(1),
    );
    assert!(schema.parse_value(&json!("  hello  ")).is_ok());
    assert!(schema.parse_value(&json!("   ")).is_err());
}
