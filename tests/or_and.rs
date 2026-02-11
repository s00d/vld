use vld::prelude::*;

#[test]
fn or_string_or_number() {
    let schema = vld::string().or(vld::number().int());
    let left = schema.parse(r#""hello""#).unwrap();
    assert!(left.is_left());
    let right = schema.parse("42").unwrap();
    assert!(right.is_right());
}

#[test]
fn or_rejects_both_invalid() {
    let schema = vld::string().or(vld::number().int());
    assert!(schema.parse("true").is_err());
}

#[test]
fn and_both_pass() {
    let schema = vld::string().min(3).and(vld::string().max(10));
    assert!(schema.parse(r#""hello""#).is_ok());
}

#[test]
fn and_first_fails() {
    let schema = vld::string().min(10).and(vld::string().max(20));
    assert!(schema.parse(r#""hi""#).is_err());
}

#[test]
fn and_second_fails() {
    let schema = vld::string().min(1).and(vld::string().max(2));
    assert!(schema.parse(r#""hello""#).is_err());
}
