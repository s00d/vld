use vld::prelude::*;

#[test]
fn custom_even_number() {
    let schema = vld::custom(|v: &serde_json::Value| {
        let n = v.as_i64().ok_or_else(|| "Expected integer".to_string())?;
        if n % 2 == 0 {
            Ok(n)
        } else {
            Err("Must be even".to_string())
        }
    });
    assert_eq!(schema.parse("4").unwrap(), 4);
    assert!(schema.parse("5").is_err());
}

#[test]
fn custom_string_validation() {
    let schema = vld::custom(|v: &serde_json::Value| {
        let s = v.as_str().ok_or_else(|| "Expected string".to_string())?;
        if s.starts_with("vld_") {
            Ok(s.to_string())
        } else {
            Err("Must start with 'vld_'".to_string())
        }
    });
    assert_eq!(schema.parse(r#""vld_test""#).unwrap(), "vld_test");
    assert!(schema.parse(r#""hello""#).is_err());
}

#[test]
fn custom_error_contains_received() {
    let schema = vld::custom(|_v: &serde_json::Value| -> Result<bool, String> {
        Err("always fails".to_string())
    });
    let err = schema.parse("42").unwrap_err();
    assert!(err.issues[0].received.is_some());
}
