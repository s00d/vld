use vld::prelude::*;

// ---------------------------------------------------------------------------
// ZString::type_error
// ---------------------------------------------------------------------------

#[test]
fn string_type_error_custom() {
    let schema = vld::string().type_error("Must be text!");
    let err = schema.parse("42").unwrap_err();
    assert_eq!(err.issues[0].message, "Must be text!");
}

#[test]
fn string_type_error_default() {
    let err = vld::string().parse("42").unwrap_err();
    assert!(err.issues[0].message.contains("Expected string"));
}

// ---------------------------------------------------------------------------
// ZString::with_messages
// ---------------------------------------------------------------------------

#[test]
fn string_with_messages_replaces() {
    let schema = vld::string().min(5).email().with_messages(|key| match key {
        "too_small" => Some("Too short!".into()),
        "invalid_email" => Some("Bad email!".into()),
        _ => None,
    });

    let err = schema.parse(r#""hi""#).unwrap_err();
    assert!(err.issues.iter().any(|i| i.message == "Too short!"));
    assert!(err.issues.iter().any(|i| i.message == "Bad email!"));
}

#[test]
fn string_with_messages_keeps_unmatched() {
    let schema = vld::string().min(5).url().with_messages(|key| match key {
        "too_small" => Some("Short!".into()),
        _ => None, // url message stays default
    });

    let err = schema.parse(r#""hi""#).unwrap_err();
    assert!(err.issues.iter().any(|i| i.message == "Short!"));
    assert!(err.issues.iter().any(|i| i.message.contains("URL")));
}

// ---------------------------------------------------------------------------
// ZNumber::type_error
// ---------------------------------------------------------------------------

#[test]
fn number_type_error_custom() {
    let schema = vld::number().type_error("Numbers only!");
    let err = schema.parse(r#""hello""#).unwrap_err();
    assert_eq!(err.issues[0].message, "Numbers only!");
}

// ---------------------------------------------------------------------------
// ZNumber::with_messages
// ---------------------------------------------------------------------------

#[test]
fn number_with_messages_replaces() {
    let schema = vld::number()
        .min(0.0)
        .max(100.0)
        .with_messages(|key| match key {
            "too_small" => Some("Must be >= 0".into()),
            "too_big" => Some("Must be <= 100".into()),
            _ => None,
        });

    let err = schema.parse("-5").unwrap_err();
    assert_eq!(err.issues[0].message, "Must be >= 0");

    let err = schema.parse("200").unwrap_err();
    assert_eq!(err.issues[0].message, "Must be <= 100");
}

// ---------------------------------------------------------------------------
// ZInt::type_error + int_error + with_messages
// ---------------------------------------------------------------------------

#[test]
fn int_type_error_custom() {
    let schema = vld::number().int().type_error("Integers only!");
    let err = schema.parse(r#""hello""#).unwrap_err();
    assert_eq!(err.issues[0].message, "Integers only!");
}

#[test]
fn int_int_error_custom() {
    let schema = vld::number().int().int_error("Whole numbers only!");
    let err = schema.parse("3.5").unwrap_err();
    assert_eq!(err.issues[0].message, "Whole numbers only!");
}

#[test]
fn int_with_messages_replaces() {
    let schema = vld::number()
        .int()
        .min(1)
        .max(10)
        .with_messages(|key| match key {
            "too_small" => Some("Min 1".into()),
            "too_big" => Some("Max 10".into()),
            "not_int" => Some("No decimals".into()),
            _ => None,
        });

    let err = schema.parse("0").unwrap_err();
    assert_eq!(err.issues[0].message, "Min 1");

    let err = schema.parse("3.5").unwrap_err();
    assert_eq!(err.issues[0].message, "No decimals");
}

// ---------------------------------------------------------------------------
// _msg variants (already existing API)
// ---------------------------------------------------------------------------

#[test]
fn string_msg_variants_work() {
    let schema = vld::string()
        .min_msg(3, "At least 3 chars")
        .email_msg("Enter a valid email");

    let err = schema.parse(r#""ab""#).unwrap_err();
    assert!(err.issues.iter().any(|i| i.message == "At least 3 chars"));
    assert!(err
        .issues
        .iter()
        .any(|i| i.message == "Enter a valid email"));
}

// ---------------------------------------------------------------------------
// IssueCode::key() and params() still work
// ---------------------------------------------------------------------------

#[test]
fn issue_code_key_and_params() {
    let code = IssueCode::TooSmall {
        minimum: 5.0,
        inclusive: true,
    };
    assert_eq!(code.key(), "too_small");
    let params = code.params();
    assert!(params.iter().any(|(k, v)| *k == "minimum" && v == "5"));
    assert!(params.iter().any(|(k, v)| *k == "inclusive" && v == "true"));
}
