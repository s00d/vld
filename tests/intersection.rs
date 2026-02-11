use vld::prelude::*;

#[test]
fn both_pass() {
    let schema = vld::intersection(vld::string().min(3), vld::string().max(10));
    assert_eq!(schema.parse(r#""hello""#).unwrap(), "hello");
}

#[test]
fn first_fails() {
    let schema = vld::intersection(vld::string().min(10), vld::string().max(20));
    assert!(schema.parse(r#""hi""#).is_err());
}

#[test]
fn second_fails() {
    let schema = vld::intersection(vld::string().min(1), vld::string().max(2));
    assert!(schema.parse(r#""hello""#).is_err());
}

#[test]
fn both_fail_errors_merged() {
    let schema = vld::intersection(vld::string().min(100), vld::string().email());
    let err = schema.parse(r#""hi""#).unwrap_err();
    assert!(err.issues.len() >= 2);
}
