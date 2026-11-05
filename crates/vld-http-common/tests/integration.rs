use vld_http_common::*;

#[test]
fn coerce_empty_stays_string() {
    assert_eq!(coerce_value(""), serde_json::json!(""));
}

#[test]
fn coerce_bool() {
    assert_eq!(coerce_value("true"), serde_json::Value::Bool(true));
    assert_eq!(coerce_value("false"), serde_json::Value::Bool(false));
    assert_eq!(coerce_value("FALSE"), serde_json::json!("FALSE"));
}

#[test]
fn coerce_null_stays_string() {
    assert_eq!(coerce_value("null"), serde_json::json!("null"));
    assert_eq!(coerce_value("NULL"), serde_json::json!("NULL"));
}

#[test]
fn coerce_int() {
    assert_eq!(coerce_value("42"), serde_json::json!(42));
    assert_eq!(coerce_value("-7"), serde_json::json!(-7));
    assert_eq!(coerce_value("0"), serde_json::json!(0));
    assert_eq!(
        coerce_value("18446744073709551615"),
        serde_json::json!(u64::MAX)
    );
}

#[test]
fn coerce_keeps_non_canonical_numbers() {
    for raw in [
        "01234",
        "+15551234567",
        "1e3",
        "-0",
        "1.50",
        "1.",
        ".5",
        "12345678901234567890123",
        "0.1000000000000000000001",
        "NaN",
        "inf",
        " 1",
    ] {
        assert_eq!(coerce_value(raw), serde_json::json!(raw), "{raw}");
    }
}

#[test]
fn coerce_float() {
    let expected: f64 = "3.14".parse().unwrap();
    assert_eq!(coerce_value("3.14"), serde_json::json!(expected));
}

#[test]
fn coerce_string() {
    assert_eq!(
        coerce_value("hello"),
        serde_json::Value::String("hello".into())
    );
}

#[test]
fn parse_qs_basic() {
    let map = parse_query_string("name=Alice&age=30&active=true");
    assert_eq!(map["name"], serde_json::json!("Alice"));
    assert_eq!(map["age"], serde_json::json!(30));
    assert_eq!(map["active"], serde_json::json!(true));
}

#[test]
fn parse_qs_empty() {
    let map = parse_query_string("");
    assert!(map.is_empty());
}

#[test]
fn parse_qs_encoded() {
    let map = parse_query_string("msg=hello+world&key=a%26b");
    assert_eq!(map["msg"], serde_json::json!("hello world"));
    assert_eq!(map["key"], serde_json::json!("a&b"));
}

#[test]
fn cookies_basic() {
    let val = cookies_to_json("session=abc123; theme=dark");
    let obj = val.as_object().unwrap();
    assert_eq!(obj["session"], serde_json::json!("abc123"));
    assert_eq!(obj["theme"], serde_json::json!("dark"));
}

#[test]
fn cookies_empty() {
    let val = cookies_to_json("");
    assert_eq!(val, serde_json::json!({}));
}

#[test]
fn url_decode_basic() {
    assert_eq!(url_decode("hello+world"), "hello world");
    assert_eq!(url_decode("a%26b"), "a&b");
    assert_eq!(url_decode("100%25"), "100%");
}

#[test]
fn url_decode_utf8_and_strict_hex() {
    assert_eq!(url_decode("caf%C3%A9"), "café");
    assert_eq!(url_decode("%4"), "%4");
    assert_eq!(url_decode("%4g"), "%4g");
    assert_eq!(url_decode("%"), "%");
    assert_eq!(url_decode("%FF"), "\u{FFFD}");
}

#[test]
fn percent_decode_keeps_plus() {
    assert_eq!(percent_decode("a+b%20c"), "a+b c");
}

#[test]
fn query_arrays() {
    let map = parse_query_string("a=1&a=2&b[]=x&c=3");
    assert_eq!(map["a"], serde_json::json!([1, 2]));
    assert_eq!(map["b"], serde_json::json!(["x"]));
    assert_eq!(map["c"], serde_json::json!(3));
    let map = parse_query_string("a[]=1&a[]=2&a=3");
    assert_eq!(map["a"], serde_json::json!([1, 2, 3]));
}

#[test]
fn query_bare_and_empty_keys() {
    let map = parse_query_string("flag&x=&n=null");
    assert_eq!(map["flag"], serde_json::json!(""));
    assert_eq!(map["x"], serde_json::json!(""));
    assert_eq!(map["n"], serde_json::json!("null"));
}

#[test]
fn cookies_quoted_and_encoded() {
    let val = cookies_to_json(r#"sid="abc"; name=caf%C3%A9; plus=a+b"#);
    assert_eq!(val["sid"], "abc");
    assert_eq!(val["name"], "café");
    assert_eq!(val["plus"], "a+b");
}

#[test]
fn headers_repeated_lossy_and_case() {
    let h: Vec<(&str, &[u8])> = vec![
        ("X-Tag", b"a"),
        ("x-tag", b"b"),
        ("X-Bin", b"caf\xC3\xA9 \xFF"),
        ("X-Num", b"42"),
    ];
    let v = headers_to_json(h);
    assert_eq!(v["x_tag"], "a, b");
    assert_eq!(v["x_bin"], "café \u{FFFD}");
    assert_eq!(v["x_num"], 42);
}

#[test]
fn path_params_coerced() {
    let v = path_params_to_json([("id", "7"), ("slug", "007")]);
    assert_eq!(v, serde_json::json!({"id": 7, "slug": "007"}));
}

#[test]
fn form_body_parsing() {
    assert_eq!(
        parse_form_body(b"a=1&b=x+y").unwrap(),
        serde_json::json!({"a": 1, "b": "x y"})
    );
    let err = parse_form_body(b"a=\xFF").unwrap_err();
    assert_eq!(err.status(), 422);
}

#[test]
fn json_body_parsing() {
    assert_eq!(
        parse_json_body(br#"{"a":1}"#).unwrap(),
        serde_json::json!({"a":1})
    );
    let err = parse_json_body(b"{").unwrap_err();
    assert_eq!(err.status(), 422);
    assert!(err.body()["issues"][0]["message"]
        .as_str()
        .unwrap()
        .starts_with("Invalid JSON"));
}

#[test]
fn json_content_type_detection() {
    assert!(is_json_content_type("application/json"));
    assert!(is_json_content_type("Application/JSON; charset=utf-8"));
    assert!(is_json_content_type("application/vnd.api+json"));
    assert!(!is_json_content_type("text/plain"));
    assert!(!is_json_content_type("application/jsonp"));
    assert!(!is_json_content_type("text/json+x"));
    assert!(is_json_content_type("application/json;charset"));
    assert!(is_json_content_type("application/json; charset=\"utf-8"));
    assert!(!is_json_content_type("application/+json"));
}

#[test]
fn form_content_type_detection() {
    assert!(require_form_content_type(Some("application/x-www-form-urlencoded")).is_ok());
    assert!(
        require_form_content_type(Some("Application/X-WWW-Form-Urlencoded; charset=UTF-8")).is_ok()
    );
    for ct in [None, Some("application/json"), Some("multipart/form-data")] {
        let e = require_form_content_type(ct).unwrap_err();
        assert_eq!(e.status(), 415);
        assert_eq!(e.body()["message"], UNSUPPORTED_FORM_MEDIA_TYPE_MESSAGE);
    }
    assert_eq!(
        require_json_content_type(None).unwrap_err().body()["message"],
        UNSUPPORTED_MEDIA_TYPE_MESSAGE
    );
}

#[test]
fn form_body_field_cap() {
    let ok = "a=1&".repeat(MAX_FORM_FIELDS);
    assert!(parse_form_body(ok.as_bytes()).is_ok());
    let err = parse_form_body(format!("{ok}b").as_bytes()).unwrap_err();
    assert_eq!(err.status(), 422);
    // 2 MiB of `a&a&…` is rejected before it expands into JSON values.
    let amplified = "a&".repeat(DEFAULT_BODY_LIMIT / 2);
    assert_eq!(
        parse_form_body(amplified.as_bytes()).unwrap_err().status(),
        422
    );
}

#[test]
fn cookies_first_occurrence_wins() {
    let val = cookies_to_json("sid=legit; sid=evil");
    assert_eq!(val["sid"], "legit");
}

#[test]
fn cookie_headers_are_joined() {
    let headers: [&[u8]; 3] = [b"a=1", b"b=2; a=3", "c=caf\u{e9}".as_bytes()];
    let val = cookie_headers_to_json(headers);
    assert_eq!(val, serde_json::json!({"a": 1, "b": 2, "c": "café"}));
    assert_eq!(cookie_headers_to_json([]), serde_json::json!({}));
}

#[test]
fn headers_with_underscore_are_skipped() {
    let h: Vec<(&str, &[u8])> = vec![("x_user", b"admin"), ("X-User", b"bob")];
    assert_eq!(headers_to_json(h), serde_json::json!({"x_user": "bob"}));
}

#[test]
fn http_error_bodies() {
    let e = VldHttpError::payload_too_large();
    assert_eq!(e.status(), 413);
    assert_eq!(e.body(), serde_json::json!({"error": "Payload too large"}));
    assert_eq!(e.to_string(), "Payload too large");
    let e = VldHttpError::parse("boom");
    assert_eq!(e.status(), 422);
    assert_eq!(e.body()["error"], "Validation failed");
    assert!(e.to_string().starts_with("Validation failed"));
}

#[test]
fn format_issues_basic() {
    let err = vld::error::VldError::single(vld::error::IssueCode::MissingField, "Required");
    let issues = format_issues(&err);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].message, "Required");
}

#[test]
#[allow(deprecated)]
fn format_issues_with_code_joins_path_with_dots() {
    use vld::error::{IssueCode, PathSegment, VldError};
    let mut err = VldError::single(IssueCode::MissingField, "Required");
    err.issues[0].path = vec![
        PathSegment::Field("user".into()),
        PathSegment::Field("age".into()),
    ];
    let issues = format_issues_with_code(&err);
    assert_eq!(issues[0].path, ".user.age");
    assert_eq!(issues[0].code, IssueCode::MissingField.key());
}

#[test]
fn format_vld_error_structure() {
    let err = vld::error::VldError::single(vld::error::IssueCode::MissingField, "Required");
    let body = format_vld_error(&err);
    assert_eq!(body["error"], "Validation failed");
    assert!(body["issues"].as_array().unwrap().len() == 1);
    assert_eq!(body["issues"][0]["message"], "Required");
}

#[test]
#[allow(deprecated)]
fn format_json_parse_error_structure() {
    let body = format_json_parse_error("unexpected token");
    assert_eq!(body["error"], "Invalid JSON");
    assert_eq!(body["message"], "unexpected token");
}

#[test]
#[allow(deprecated)]
fn format_utf8_error_structure() {
    let body = format_utf8_error();
    assert_eq!(body["error"], "Invalid UTF-8");
}

#[test]
fn format_payload_too_large_structure() {
    let body = format_payload_too_large();
    assert_eq!(body["error"], "Payload too large");
}

#[test]
fn format_generic_error_structure() {
    let body = format_generic_error("Not Found");
    assert_eq!(body["error"], "Not Found");
}

#[test]
fn error_body_schema_roundtrip() {
    let body = ErrorBody {
        error: "test".into(),
    };
    let json = serde_json::to_value(&body).unwrap();
    assert_eq!(json["error"], "test");
    let parsed = ErrorBody::parse_value(&json).unwrap();
    assert_eq!(parsed.error, "test");
}

#[test]
fn validation_error_body_schema_roundtrip() {
    let body = ValidationErrorBody {
        error: "Validation failed".into(),
        issues: vec![ValidationIssue {
            path: "name".into(),
            message: "too short".into(),
            code: "too_small".into(),
        }],
    };
    let json = serde_json::to_value(&body).unwrap();
    assert_eq!(json["error"], "Validation failed");
    assert_eq!(json["issues"][0]["path"], "name");
    let parsed = ValidationErrorBody::parse_value(&json).unwrap();
    assert_eq!(parsed.issues.len(), 1);
    assert_eq!(parsed.issues[0].message, "too short");
}
