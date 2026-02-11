use vld::prelude::*;

#[test]
fn safe_accepts_normal_numbers() {
    let schema = vld::number().safe();
    assert!(schema.parse("42").is_ok());
    assert!(schema.parse("9007199254740991").is_ok()); // 2^53 - 1
    assert!(schema.parse("-9007199254740991").is_ok());
}

#[test]
fn safe_rejects_too_large() {
    let schema = vld::number().safe();
    assert!(schema.parse("9007199254740992").is_err()); // 2^53
}

#[test]
fn int_safe() {
    let schema = vld::number().int().safe();
    assert!(schema.parse("42").is_ok());
    assert!(schema.parse("9007199254740992").is_err());
}
