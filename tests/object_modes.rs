use vld::prelude::*;

#[test]
fn strip_is_default() {
    let schema = vld::object().field("name", vld::string());
    let result = schema.parse(r#"{"name":"Alex","extra":true}"#).unwrap();
    assert!(result.get("name").is_some());
    assert!(result.get("extra").is_none());
}

#[test]
fn passthrough_keeps_unknown() {
    let schema = vld::object().field("name", vld::string()).passthrough();
    let result = schema.parse(r#"{"name":"Alex","extra":true}"#).unwrap();
    assert!(result.get("name").is_some());
    assert!(result.get("extra").is_some());
}

#[test]
fn strict_rejects_unknown() {
    let schema = vld::object().field("name", vld::string()).strict();
    let err = schema.parse(r#"{"name":"Alex","extra":true}"#).unwrap_err();
    assert!(err
        .issues
        .iter()
        .any(|i| i.message.contains("Unrecognized")));
}

#[test]
fn pick_keeps_selected() {
    let schema = vld::object()
        .field("a", vld::string())
        .field("b", vld::number())
        .field("c", vld::boolean())
        .pick(&["a", "c"]);
    let result = schema.parse(r#"{"a":"x","c":true}"#).unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn omit_removes_field() {
    let schema = vld::object()
        .field("a", vld::string())
        .field("b", vld::number())
        .omit("b");
    let result = schema.parse(r#"{"a":"x"}"#).unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn extend_merges_schemas() {
    let base = vld::object().field("a", vld::string());
    let extra = vld::object().field("b", vld::number());
    let schema = base.extend(extra);
    let result = schema.parse(r#"{"a":"x","b":42}"#).unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn merge_overrides_fields() {
    let base = vld::object().field("a", vld::string().min(1));
    let override_schema = vld::object().field("a", vld::string().min(10));
    let schema = base.merge(override_schema);
    assert!(schema.parse(r#"{"a":"hi"}"#).is_err());
}

#[test]
fn partial_makes_fields_optional() {
    let schema = vld::object()
        .field("name", vld::string().min(1))
        .field("age", vld::number().int())
        .partial();
    // null fields should pass after partial()
    let result = schema.parse(r#"{"name":null,"age":null}"#).unwrap();
    assert!(result.get("name").is_some());
    assert!(result.get("age").is_some());
}
