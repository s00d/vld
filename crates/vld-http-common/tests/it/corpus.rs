//! Corpora: fixed inputs with expected verdicts.

use serde_json::{Value, json};
use vld_http_common::*;

/// Text → JSON coercion of documented examples (the model in `model_text`
/// generates the rest).
#[test]
fn coerce_value_examples() {
    let cases: &[(&str, Value)] = &[
        ("true", json!(true)),
        ("false", json!(false)),
        ("42", json!(42)),
        ("-7", json!(-7)),
        ("0", json!(0)),
        ("18446744073709551615", json!(u64::MAX)),
        ("2.5", json!(2.5_f64)),
        ("-0.5", json!(-0.5_f64)),
        ("hello", json!("hello")),
        ("", json!("")),
        ("FALSE", json!("FALSE")),
        ("TRUE", json!("TRUE")),
        ("null", json!("null")),
        ("NULL", json!("NULL")),
        ("01234", json!("01234")),
        ("+15551234567", json!("+15551234567")),
        ("1e3", json!("1e3")),
        ("-0", json!("-0")),
        ("1.50", json!("1.50")),
        ("1.", json!("1.")),
        (".5", json!(".5")),
        ("12345678901234567890123", json!("12345678901234567890123")),
        (
            "0.1000000000000000000001",
            json!("0.1000000000000000000001"),
        ),
        ("NaN", json!("NaN")),
        ("inf", json!("inf")),
        (" 1", json!(" 1")),
    ];
    for (raw, want) in cases {
        assert_eq!(&coerce_value(raw), want, "{raw:?}");
    }
}

/// Which `Content-Type` values the JSON and form extractors accept; every
/// rejection is a 415 whose message names the expected media type.
#[test]
fn content_type_verdicts() {
    let json_cases: &[(&str, bool)] = &[
        ("application/json", true),
        ("Application/JSON; charset=utf-8", true),
        ("  application/json  ", true),
        ("application/vnd.api+json", true),
        ("application/problem+json; charset=utf-8", true),
        ("application/json;charset", true),
        ("application/json; charset=\"utf-8", true),
        ("application/+json", false),
        ("application/jsonp", false),
        ("text/json+x", false),
        ("text/json", false),
        ("text/plain", false),
        ("application/x-www-form-urlencoded", false),
        ("", false),
    ];
    for (ct, want) in json_cases {
        assert_eq!(is_json_content_type(ct), *want, "{ct:?}");
        let got = require_json_content_type(Some(ct));
        assert_eq!(got.is_ok(), *want, "{ct:?}");
        if let Err(e) = got {
            assert_eq!(e.status(), 415);
            assert_eq!(
                e.body(),
                json!({"error": "Unsupported Media Type", "message": UNSUPPORTED_MEDIA_TYPE_MESSAGE})
            );
        }
    }
    assert_eq!(require_json_content_type(None).unwrap_err().status(), 415);

    let form_cases: &[(Option<&str>, bool)] = &[
        (Some("application/x-www-form-urlencoded"), true),
        (
            Some("Application/X-WWW-Form-Urlencoded; charset=UTF-8"),
            true,
        ),
        (Some("application/json"), false),
        (Some("multipart/form-data"), false),
        (Some("application/x-www-form-urlencoded+json"), false),
        (None, false),
    ];
    for (ct, want) in form_cases {
        let got = require_form_content_type(*ct);
        assert_eq!(got.is_ok(), *want, "{ct:?}");
        if let Err(e) = got {
            assert_eq!(e.status(), 415);
            assert_eq!(
                e.body(),
                json!({"error": "Unsupported Media Type", "message": UNSUPPORTED_FORM_MEDIA_TYPE_MESSAGE})
            );
        }
    }
}

/// Percent decoding: strict two-hex-digit escapes, UTF-8, `U+FFFD` for
/// invalid bytes; `url_decode` also turns `+` into a space.
#[test]
fn decoding_examples() {
    let cases: &[(&str, &str, &str)] = &[
        // (input, url_decode, percent_decode)
        ("hello+world", "hello world", "hello+world"),
        ("a%26b", "a&b", "a&b"),
        ("100%25", "100%", "100%"),
        ("caf%C3%A9", "café", "café"),
        ("%c3%a9", "é", "é"),
        ("%4", "%4", "%4"),
        ("%4g", "%4g", "%4g"),
        ("%g4", "%g4", "%g4"),
        ("%", "%", "%"),
        ("%%41", "%A", "%A"),
        ("%FF", "\u{FFFD}", "\u{FFFD}"),
        ("a+b%20c", "a b c", "a+b c"),
        ("%2B", "+", "+"),
    ];
    for (input, url, pct) in cases {
        assert_eq!(url_decode(input), *url, "url_decode({input:?})");
        assert_eq!(percent_decode(input), *pct, "percent_decode({input:?})");
    }
}
