use vld::prelude::*;

// ---- Map ----

#[test]
fn map_basic() {
    let schema = vld::map(vld::string(), vld::number().int());
    let result = schema.parse(r#"[["a", 1], ["b", 2]]"#).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result.get("a"), Some(&1));
    assert_eq!(result.get("b"), Some(&2));
}

#[test]
fn map_invalid_entry_format() {
    let schema = vld::map(vld::string(), vld::number().int());
    assert!(schema.parse(r#"[["a", 1], "bad"]"#).is_err());
}

#[test]
fn map_value_validation_fails() {
    let schema = vld::map(vld::string(), vld::number().int().positive());
    assert!(schema.parse(r#"[["a", -1]]"#).is_err());
}

#[test]
fn map_key_validation_fails() {
    let schema = vld::map(vld::string().min(3), vld::number().int());
    assert!(schema.parse(r#"[["ab", 1]]"#).is_err());
}

#[test]
fn map_empty() {
    let schema = vld::map(vld::string(), vld::number());
    let result = schema.parse("[]").unwrap();
    assert!(result.is_empty());
}

#[test]
fn map_not_array() {
    let schema = vld::map(vld::string(), vld::number());
    assert!(schema.parse(r#"{"a":1}"#).is_err());
}

// ---- Set ----

#[test]
fn set_basic() {
    let schema = vld::set(vld::string().min(1));
    let result = schema.parse(r#"["a", "b", "c"]"#).unwrap();
    assert_eq!(result.len(), 3);
    assert!(result.contains("a"));
}

#[test]
fn set_deduplicates() {
    let schema = vld::set(vld::string());
    let result = schema.parse(r#"["a", "b", "a"]"#).unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn set_min_size() {
    let schema = vld::set(vld::number().int()).min_size(2);
    assert!(schema.parse("[1, 2]").is_ok());
    assert!(schema.parse("[1]").is_err());
}

#[test]
fn set_max_size() {
    let schema = vld::set(vld::number().int()).max_size(2);
    assert!(schema.parse("[1, 2]").is_ok());
    assert!(schema.parse("[1, 2, 3]").is_err());
}

#[test]
fn set_element_validation() {
    let schema = vld::set(vld::string().non_empty());
    assert!(schema.parse(r#"["a", ""]"#).is_err());
}

#[test]
fn set_empty() {
    let schema = vld::set(vld::string());
    let result = schema.parse("[]").unwrap();
    assert!(result.is_empty());
}

#[test]
fn set_not_array() {
    let schema = vld::set(vld::string());
    assert!(schema.parse(r#""hello""#).is_err());
}
