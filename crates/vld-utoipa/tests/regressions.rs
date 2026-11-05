use serde_json::{json, Value};
use vld_utoipa::utoipa::openapi::path::ParameterIn;
use vld_utoipa::{json_schema_to_params, json_schema_to_schema};

fn conv(v: Value) -> Value {
    serde_json::to_value(json_schema_to_schema(&v)).unwrap()
}

#[test]
fn ref_keeps_description_and_wraps_default() {
    let only_desc = conv(json!({"$ref": "#/components/schemas/A", "description": "addr"}));
    assert_eq!(only_desc["$ref"], "#/components/schemas/A");
    assert_eq!(only_desc["description"], "addr");

    let with_default = conv(json!({
        "$ref": "#/components/schemas/A", "description": "addr", "default": {"x": 1}, "title": "T"
    }));
    assert_eq!(with_default["allOf"][0]["$ref"], "#/components/schemas/A");
    assert_eq!(with_default["description"], "addr");
    assert_eq!(with_default["default"], json!({"x": 1}));
    assert_eq!(with_default["title"], "T");
}

#[test]
fn composites_keep_annotations() {
    for key in ["oneOf", "anyOf", "allOf"] {
        let out = conv(json!({
            key: [{"type": "string"}, {"type": "null"}],
            "description": "d", "default": "x"
        }));
        assert_eq!(out[key].as_array().unwrap().len(), 2, "{key}");
        assert_eq!(out["description"], "d", "{key}");
        assert_eq!(out["default"], "x", "{key}");
    }
}

#[test]
fn missing_type_is_any_value() {
    let out = conv(json!({}));
    assert!(out.get("type").is_none(), "{out}");
    let out = conv(json!({"description": "anything"}));
    assert!(out.get("type").is_none(), "{out}");
    assert_eq!(out["description"], "anything");
}

#[test]
fn type_arrays_are_preserved() {
    let out = conv(json!({"type": ["string", "null"], "minLength": 2}));
    assert_eq!(out["type"], json!(["string", "null"]));
    assert_eq!(out["minLength"], 2);
    let arr = conv(json!({"type": ["array", "null"], "items": {"type": "integer"}}));
    assert_eq!(arr["type"], json!(["array", "null"]));
    assert_eq!(arr["items"]["type"], "integer");
}

#[test]
fn array_keeps_annotations_and_unique_items() {
    let out = conv(json!({
        "type": "array", "items": {"type": "string"}, "uniqueItems": true,
        "description": "tags", "default": [], "title": "Tags"
    }));
    assert_eq!(out["uniqueItems"], true);
    assert_eq!(out["description"], "tags");
    assert_eq!(out["default"], json!([]));
    assert_eq!(out["title"], "Tags");
}

#[test]
fn object_keeps_additional_and_property_counts() {
    let out = conv(json!({
        "type": "object", "additionalProperties": {"type": "integer"},
        "minProperties": 1, "maxProperties": 3
    }));
    assert_eq!(out["additionalProperties"]["type"], "integer");
    assert_eq!(out["minProperties"], 1);
    assert_eq!(out["maxProperties"], 3);
    let closed = conv(json!({"type": "object", "properties": {}, "additionalProperties": false}));
    assert_eq!(closed["additionalProperties"], false);
}

#[test]
fn const_becomes_single_enum() {
    let out = conv(json!({"type": "string", "const": "x"}));
    assert_eq!(out["enum"], json!(["x"]));
}

#[test]
fn record_from_vld_keeps_value_type() {
    use vld::openapi::JsonSchema;
    let s = vld::record(vld::number().int()).json_schema();
    let out = conv(s);
    assert_eq!(out["additionalProperties"]["type"], "integer");
}

#[test]
fn params_take_outer_description_and_default() {
    let schema = json!({
        "type": "object",
        "properties": {
            "page": {
                "anyOf": [{"type": "integer", "minimum": 1}, {"type": "null"}],
                "description": "page number",
                "default": 1
            },
            "q": {
                "oneOf": [{"type": "string"}, {"type": "null"}],
                "description": "query"
            }
        }
    });
    let params = json_schema_to_params(&schema, ParameterIn::Query);
    let params = serde_json::to_value(&params).unwrap();
    let page = params
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "page")
        .unwrap();
    assert_eq!(page["description"], "page number");
    assert_eq!(page["schema"]["type"], "integer");
    assert_eq!(page["schema"]["minimum"], 1);
    assert_eq!(page["schema"]["default"], 1);
    let q = params
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "q")
        .unwrap();
    assert_eq!(q["description"], "query");
    assert_eq!(q["schema"]["type"], "string");
}

#[test]
fn optional_ref_any_of_keeps_ref_and_is_not_object() {
    let out = conv(json!({"anyOf": [{"$ref": "#/components/schemas/Address"}, {"type": "null"}]}));
    assert!(out.get("type").is_none(), "{out}");
    assert_eq!(out["anyOf"][0]["$ref"], "#/components/schemas/Address");
    let obj = conv(json!({"type": "object", "properties": {
        "addr": {"anyOf": [{"$ref": "#/components/schemas/Address"}, {"type": "null"}]}}}));
    assert_eq!(
        obj["properties"]["addr"]["anyOf"][0]["$ref"],
        "#/components/schemas/Address"
    );
}
