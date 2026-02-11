use vld::prelude::*;

#[test]
fn describe_preserves_validation() {
    let schema = vld::string().min(3).describe("User's full name");
    assert!(schema.parse(r#""Alex""#).is_ok());
    assert!(schema.parse(r#""Al""#).is_err());
}

#[test]
fn describe_stores_text() {
    let schema = vld::number().min(0.0).describe("Non-negative score");
    assert_eq!(schema.description(), "Non-negative score");
}
