//! Story: a sign-up form keeps the password out of every error surface —
//! issues, `Display`, `Debug`, the serialized error, translations, custom
//! messages — while paths, codes and messages stay usable.

use serde_json::json;
use vld::error::{IssueCode, REDACTED, StringValidation as V};
use vld::prelude::*;

const SECRET: &str = "correcthorsebat";

fn password() -> impl vld::Schema<Output = String> {
    vld::string()
        .min(16)
        .require_uppercase()
        .require_digit()
        .require_symbol()
        .max_repeat(2)
        .with_messages(|k| (k == "invalid_password_symbol").then(|| "Add a symbol".into()))
        .redact()
}

/// No error surface carries the secret.
fn assert_secret_free(err: &VldError) {
    #[cfg_attr(not(feature = "i18n"), allow(unused_mut))]
    let mut texts = vec![
        err.to_string(),
        format!("{err:?}"),
        serde_json::to_string(err).unwrap(),
        vld::format::prettify_error(err),
        vld::i18n::translate_error(err, &vld::i18n::english()).to_string(),
    ];
    #[cfg(feature = "i18n")]
    texts.push(vld::i18n::translate_error(err, &vld::i18n::translator("ru")).to_string());
    for text in texts {
        assert!(!text.contains(SECRET), "{text}");
    }
}

/// Given a sign-up object whose password field is `redact()`ed (and a
/// password history whose `unique()` check sits outside the field),
/// when a weak password, a nested list of secrets and a refinement message
/// that quotes the value are parsed,
/// then every issue keeps its path, code and message, its `received` is the
/// marker, and no error surface contains the secret.
#[test]
fn password_never_appears_in_errors() {
    let signup = vld::object()
        .field("user", vld::string().min(2))
        .field("password", password());

    // Step 1: one issue per failed password rule, at the field's path.
    let body = json!({"user": "ada", "password": SECRET});
    let err = signup.parse_value(&body).unwrap_err();
    let got: Vec<(String, IssueCode)> = err
        .issues
        .iter()
        .map(|i| (i.path_string(), i.code.clone()))
        .collect();
    let invalid = |validation| IssueCode::InvalidString { validation };
    assert_eq!(
        got,
        [
            (
                ".password".to_string(),
                IssueCode::TooSmall {
                    origin: vld::error::SizeOrigin::String,
                    minimum: json!(16),
                    inclusive: true
                }
            ),
            (".password".to_string(), invalid(V::PasswordUppercase)),
            (".password".to_string(), invalid(V::PasswordDigit)),
            (".password".to_string(), invalid(V::PasswordSymbol)),
        ]
    );
    for issue in &err.issues {
        assert_eq!(issue.received, Some(json!(REDACTED)));
    }
    // `with_messages` still applies; messages name the rule, not the value.
    assert_eq!(err.issues[3].message, "Add a symbol");
    assert_eq!(
        err.issues[1].message,
        "Password must contain an uppercase letter"
    );
    assert!(err.to_string().contains(", received \"[redacted]\""));
    assert_secret_free(&err);

    // Step 2: a wrong type is redacted too (the type name is not the value).
    let err = signup
        .parse_value(&json!({"user": "ada", "password": 12345678}))
        .unwrap_err();
    assert_eq!(err.issues[0].code.key(), "invalid_type");
    assert_eq!(err.issues[0].received, Some(json!(REDACTED)));

    // Step 3: a redacted subtree redacts nested issues (array items, object
    // fields) and a refinement whose message quotes the value.
    let keys = vld::array(
        vld::string()
            .min(4)
            .try_transform(|s| match s.starts_with("sk_") {
                true => Ok(s),
                false => Err(format!("`{s}` is not a secret key")),
            }),
    )
    .redact();
    let err = keys
        .parse_value(&json!(["sk_live_1", SECRET, "abc"]))
        .unwrap_err();
    let paths: Vec<String> = err.issues.iter().map(|i| i.path_string()).collect();
    assert_eq!(paths, ["[1]", "[2]"]);
    assert_eq!(err.issues[0].message, "`[redacted]` is not a secret key");
    // The transform error carries no value; the length issue the marker.
    assert_eq!(err.issues[0].received, None);
    assert_eq!(err.issues[1].received, Some(json!(REDACTED)));
    assert_secret_free(&err);

    // Step 4: a check of the enclosing schema (`unique()`, a union) reports
    // the value it saw as received, outside the redacted field; redacting
    // the enclosing schema keeps the secret out of it.
    let history = json!([SECRET, SECRET]);
    let leaky = vld::array(vld::string().redact()).unique();
    let err = leaky.parse_value(&history).unwrap_err();
    assert_eq!(err.issues[0].code, IssueCode::NotUnique);
    assert!(err.to_string().contains(SECRET));
    let err = vld::array(vld::string())
        .unique()
        .redact()
        .parse_value(&history)
        .unwrap_err();
    assert_eq!(err.issues[0].code, IssueCode::NotUnique);
    assert_eq!(err.issues[0].path_string(), "[1]");
    assert_secret_free(&err);

    // Step 5: valid input is untouched, and the JSON Schema is the inner one.
    let ok = signup
        .parse_value(&json!({"user": "ada", "password": "Correct-Horse-9-Battery"}))
        .unwrap();
    assert_eq!(ok.get("password"), Some(&json!("Correct-Horse-9-Battery")));
    #[cfg(feature = "openapi")]
    {
        let js = password().json_schema();
        assert_eq!(js["format"], "password");
        assert_eq!(
            js["x-password"],
            json!({"uppercase": true, "digit": true, "symbol": true, "maxRepeat": 2})
        );
        assert_eq!(js["minLength"], 16);
    }
}
