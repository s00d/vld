//! Story: inspecting and combining schemas — list the properties of a vld
//! type, look fields up, compare, merge with `allOf` and overlay constraints,
//! then validate with the combined schema.

use super::bridge::*;
use crate::common::{invalid, valid};
use serde_json::json;

vld::schema! {
    #[derive(Debug)]
    pub struct UserSchema {
        pub name: String  => vld::string().min(2).max(100),
        pub email: String => vld::string().email(),
        pub age: Option<i64> => vld::number().int().min(0).max(150).optional(),
    }
}

/// Given a vld type, when a tool inspects its JSON Schema, then it sees every
/// property with its type, requiredness and raw schema, and field lookups
/// agree with that list.
#[test]
fn inspect_a_vld_type() {
    let js = UserSchema::json_schema();
    let props = list_properties(&js);
    let summary: Vec<(&str, Option<&str>, bool)> = props
        .iter()
        .map(|p| (p.name.as_str(), p.schema_type.as_deref(), p.required))
        .collect();
    assert_eq!(
        summary,
        [
            ("age", Some("integer"), false),
            ("email", Some("string"), true),
            ("name", Some("string"), true)
        ]
    );
    for p in &props {
        assert_eq!(Some(&p.schema), get_property(&js, &p.name));
        assert_eq!(p.required, is_required(&js, &p.name));
    }
    assert_eq!(list_properties_schemars(&vld_to_schemars(&js)), props);
    assert_eq!(get_property(&js, "name").unwrap()["minLength"], 2);
    assert!(get_property(&js, "nonexistent").is_none());
    assert!(!is_required(&js, "nonexistent"));
    assert_eq!(schema_type(&js), Some("object".into()));
    assert_eq!(
        schema_type(&json!({"type": "string"})),
        Some("string".into())
    );
    assert_eq!(schema_type(&json!({})), None);
    assert_eq!(schema_type(&json!({"type": ["string", "null"]})), None);

    // Schemas without properties / required lists.
    assert!(list_properties(&json!({"type": "string"})).is_empty());
    assert!(get_property(&json!({"type": "object", "properties": {}}), "x").is_none());
    assert!(!is_required(&json!({"required": "name"}), "name"));
    let loose = json!({"properties": {"a": {}}, "required": [1, "a"]});
    assert_eq!(
        list_properties(&loose),
        [PropertyInfo {
            name: "a".into(),
            schema_type: None,
            required: true,
            schema: json!({})
        }]
    );
}

/// Given two schemas, when a tool compares, merges and overlays them, then
/// equality is structural, a merge is an `allOf` that enforces both sides,
/// and an overlay adds constraints without overwriting the base.
#[test]
fn combine_schemas_then_validate() {
    use vld::openapi::JsonSchema;
    assert!(schemas_equal(
        &vld::string().min(1).json_schema(),
        &json!({"type": "string", "minLength": 1})
    ));
    assert!(!schemas_equal(
        &json!({"type": "string"}),
        &json!({"type": "integer"})
    ));

    let a = vld_to_schemars(
        &json!({"type": "object", "required": ["x"], "properties": {"x": {"type": "string"}}}),
    );
    let b = vld_to_schemars(&json!({"type": "object", "properties": {"y": {"type": "integer"}}}));
    let merged = merge_schemas(&a, &b);
    assert_eq!(
        schemars_to_json(&merged),
        json!({"allOf": [schemars_to_json(&a), schemars_to_json(&b)]})
    );
    validate_with_schemars(&merged, &json!({"x": "s", "y": 1})).unwrap();
    invalid(
        merged.as_value(),
        &json!({"y": "no"}),
        &[(".x", "missing_field"), (".y", "invalid_type")],
    );

    let base = json!({"type": "object", "required": ["name"], "properties": {"name": {"type": "string", "minLength": 5}}});
    let overlay = json!({
        "properties": {"name": {"minLength": 2, "maxLength": 8}, "age": {"type": "integer"}},
        "required": ["name", "age"],
        "type": "array",
        "description": "person"
    });
    let result = overlay_constraints(&base, &overlay);
    assert_eq!(
        result,
        json!({
            "type": "object",
            "description": "person",
            "required": ["name", "age"],
            "properties": {
                "name": {"type": "string", "minLength": 5, "maxLength": 8},
                "age": {"type": "integer"}
            }
        })
    );
    valid(&result, &json!({"name": "abcde", "age": 1}));
    invalid(
        &result,
        &json!({"name": "abcdefghi"}),
        &[(".age", "missing_field"), (".name", "too_big")],
    );

    // Overlay onto a base without properties / required.
    let bare = overlay_constraints(
        &json!({"type": "object"}),
        &json!({"required": ["a"], "properties": {"a": {}}}),
    );
    assert_eq!(bare, json!({"type": "object", "required": ["a"]}));
    // A non-object property schema on either side keeps the base one.
    let odd = overlay_constraints(
        &json!({"properties": {"a": true}}),
        &json!({"properties": {"a": {"minimum": 1}}, "required": "a"}),
    );
    assert_eq!(odd, json!({"properties": {"a": true}}));
    // A non-object base or overlay is returned as the base.
    assert_eq!(
        overlay_constraints(&json!(true), &json!({"type": "x"})),
        json!(true)
    );
    assert_eq!(
        overlay_constraints(&json!({"a": 1}), &json!(false)),
        json!({"a": 1})
    );
}
