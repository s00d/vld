use vld_http_common::*;

#[test]
fn coerce_empty() {
    assert_eq!(coerce_value(""), serde_json::Value::Null);
}

#[test]
fn coerce_bool() {
    assert_eq!(coerce_value("true"), serde_json::Value::Bool(true));
    assert_eq!(coerce_value("FALSE"), serde_json::Value::Bool(false));
}

#[test]
fn coerce_null() {
    assert_eq!(coerce_value("null"), serde_json::Value::Null);
    assert_eq!(coerce_value("NULL"), serde_json::Value::Null);
}

#[test]
fn coerce_int() {
    assert_eq!(coerce_value("42"), serde_json::json!(42));
    assert_eq!(coerce_value("-7"), serde_json::json!(-7));
}

#[test]
fn coerce_float() {
    assert_eq!(coerce_value("3.14"), serde_json::json!(3.14));
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
fn extract_params() {
    let names = extract_path_param_names("/users/{id}/posts/{post_id}");
    assert_eq!(names, vec!["id", "post_id"]);
}

#[test]
fn format_issues_basic() {
    let err = vld::error::VldError::single(vld::error::IssueCode::MissingField, "Required");
    let issues = format_issues(&err);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].message, "Required");
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
fn format_json_parse_error_structure() {
    let body = format_json_parse_error("unexpected token");
    assert_eq!(body["error"], "Invalid JSON");
    assert_eq!(body["message"], "unexpected token");
}

#[test]
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
        }],
    };
    let json = serde_json::to_value(&body).unwrap();
    assert_eq!(json["error"], "Validation failed");
    assert_eq!(json["issues"][0]["path"], "name");
    let parsed = ValidationErrorBody::parse_value(&json).unwrap();
    assert_eq!(parsed.issues.len(), 1);
    assert_eq!(parsed.issues[0].message, "too short");
}
