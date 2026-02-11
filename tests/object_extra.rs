use vld::prelude::*;

#[test]
fn required_rejects_null() {
    let schema = vld::object()
        .field("name", vld::string())
        .partial()
        .required(); // undo partial
    let err = schema.parse(r#"{"name": null}"#).unwrap_err();
    assert!(err.issues.iter().any(|i| i.message.contains("Required")));
}

#[test]
fn catchall_validates_unknown_fields() {
    let schema = vld::object()
        .field("name", vld::string())
        .catchall(vld::number().int());

    // Unknown field "score" validated as int
    let result = schema.parse(r#"{"name":"Alex","score":42}"#).unwrap();
    assert_eq!(result["score"], serde_json::json!(42));

    // Unknown field "score" fails int validation
    assert!(schema.parse(r#"{"name":"Alex","score":"nope"}"#).is_err());
}

#[test]
fn catchall_overrides_unknown_mode() {
    let schema = vld::object()
        .field("a", vld::string())
        .strict()
        .catchall(vld::string());

    // With catchall, strict doesn't reject unknown fields — they go through catchall
    let result = schema.parse(r#"{"a":"x","b":"y"}"#).unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn deep_partial_same_as_partial() {
    let schema = vld::object()
        .field("name", vld::string().min(1))
        .deep_partial();
    // null should pass
    let result = schema.parse(r#"{"name": null}"#).unwrap();
    assert!(result.get("name").is_some());
}

#[test]
fn keyof_returns_field_names() {
    let schema = vld::object()
        .field("name", vld::string())
        .field("age", vld::number())
        .field("email", vld::string());

    let keys = schema.keyof();
    assert_eq!(keys.len(), 3);
    assert!(keys.contains(&"name".to_string()));
    assert!(keys.contains(&"age".to_string()));
    assert!(keys.contains(&"email".to_string()));
}
