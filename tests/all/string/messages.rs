//! Custom messages on string schemas: `*_msg`, `type_error`, `with_messages`.

use crate::common::{codes, parse_err};
use serde_json::json;
use vld::prelude::*;

/// Each `*_msg` variant replaces exactly its own issue's message and keeps the code.
#[test]
fn msg_variants_replace_message_and_keep_code() {
    let s = vld::string;
    let cases: Vec<(ZString, &str, &str)> = vec![
        (s().min_msg(3, "m"), "ab", "too_small"),
        (s().max_msg(1, "m"), "ab", "too_big"),
        (s().len_msg(5, "m"), "ab", "invalid_length"),
        (s().non_empty_msg("m"), "", "too_small"),
        (s().starts_with_msg("x", "m"), "ab", "invalid_string"),
        (s().ends_with_msg("x", "m"), "ab", "invalid_string"),
        (s().contains_msg("x", "m"), "ab", "invalid_string"),
        (s().email_msg("m"), "nope", "invalid_string"),
        (s().url_msg("m"), "nope", "invalid_string"),
        (s().uuid_msg("m"), "nope", "invalid_string"),
        (s().ipv4_msg("m"), "nope", "invalid_string"),
        (s().ipv6_msg("m"), "nope", "invalid_string"),
        (s().base64_msg("m"), "!!", "invalid_string"),
        (s().iso_date_msg("m"), "nope", "invalid_string"),
        (s().iso_time_msg("m"), "nope", "invalid_string"),
        (s().iso_datetime_msg("m"), "nope", "invalid_string"),
        (s().hostname_msg("m"), "-nope", "invalid_string"),
        (s().cuid2_msg("m"), "!!", "invalid_string"),
        (s().ulid_msg("m"), "short", "invalid_string"),
        (s().nanoid_msg("m"), "hello world", "invalid_string"),
        (s().emoji_msg("m"), "plain", "invalid_string"),
        (s().slug_msg("m"), "No Slug", "invalid_string"),
        (s().cron_msg("m"), "x", "invalid_string"),
        (s().base64url_msg("m"), "+", "invalid_string"),
    ];
    for (schema, input, code) in cases {
        let err = parse_err(&schema, json!(input));
        assert_eq!(err.issues.len(), 1, "{input:?}: {err}");
        assert_eq!(err.issues[0].message, "m", "{input:?}");
        assert_eq!(codes(&err), [code], "{input:?}");
    }
}

#[cfg(feature = "regex")]
#[test]
fn regex_msg() {
    let re = vld::regex_lite::Regex::new("^a+$").unwrap();
    let err = parse_err(&vld::string().regex_msg(re, "only a"), json!("b"));
    assert_eq!(err.issues[0].message, "only a");
}

#[cfg(feature = "string-advanced")]
#[test]
fn advanced_msg_variants() {
    let err = parse_err(
        &vld::string().uuid_v1_msg("v1 only"),
        json!("550e8400-e29b-41d4-a716-446655440000"),
    );
    assert_eq!(err.issues[0].message, "v1 only");
    let err = parse_err(
        &vld::string().url_strict_msg("public"),
        json!("http://10.0.0.1/"),
    );
    assert_eq!(err.issues[0].message, "public");
}

#[test]
fn msg_applies_only_to_its_own_check() {
    let schema = vld::string().min_msg(3, "At least 3 chars").email();
    let err = parse_err(&schema, json!("ab"));
    let messages: Vec<&str> = err.issues.iter().map(|i| i.message.as_str()).collect();
    assert_eq!(messages[0], "At least 3 chars");
    assert_ne!(messages[1], "At least 3 chars");
}

#[test]
fn default_messages() {
    let err = parse_err(&vld::string(), json!(42));
    assert_eq!(err.issues[0].message, "Expected string, received number");
    let err = parse_err(&vld::string().min(5), json!("ab"));
    assert_eq!(
        err.issues[0].message,
        "String must be at least 5 characters"
    );
    let err = parse_err(&vld::string().base64url(), json!("+"));
    assert_eq!(err.issues[0].message, "Invalid Base64URL string");
    let err = parse_err(&vld::string().json(), json!("{"));
    assert_eq!(err.issues[0].message, "Invalid JSON string");
    let schema = vld::string()
        .json()
        .with_messages(|k| (k == "invalid_json").then(|| "json!".into()));
    assert_eq!(parse_err(&schema, json!("{")).issues[0].message, "json!");
}

#[test]
fn type_error_only_replaces_the_type_issue() {
    let schema = vld::string().min(5).type_error("Must be text!");
    assert_eq!(
        parse_err(&schema, json!(42)).issues[0].message,
        "Must be text!"
    );
    assert_ne!(
        parse_err(&schema, json!("ab")).issues[0].message,
        "Must be text!"
    );
}

#[test]
fn with_messages_by_key() {
    let schema = vld::string()
        .min(5)
        .email()
        .url()
        .with_messages(|key| match key {
            "too_small" => Some("Too short!".into()),
            "invalid_email" => Some("Bad email!".into()),
            _ => None,
        });
    let err = parse_err(&schema, json!("hi"));
    let messages: Vec<&str> = err.issues.iter().map(|i| i.message.as_str()).collect();
    assert_eq!(messages[..2], ["Too short!", "Bad email!"]);
    assert!(
        messages[2].contains("URL"),
        "unmatched key keeps default: {}",
        messages[2]
    );
    assert_eq!(
        codes(&err),
        ["too_small", "invalid_string", "invalid_string"]
    );
}
