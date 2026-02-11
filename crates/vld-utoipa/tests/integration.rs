use serde_json::json;
use utoipa::openapi::RefOr;
use utoipa::{PartialSchema, ToSchema};
use vld::prelude::*;
use vld_utoipa::{impl_to_schema, json_schema_to_schema};

// ---- json_schema_to_schema tests ----

#[test]
fn string_type() {
    let s = json_schema_to_schema(&json!({"type": "string"}));
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["type"], "string");
}

#[test]
fn string_with_constraints() {
    let s = json_schema_to_schema(&json!({
        "type": "string",
        "minLength": 2,
        "maxLength": 50,
        "format": "email"
    }));
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["type"], "string");
    assert_eq!(json["minLength"], 2);
    assert_eq!(json["maxLength"], 50);
    assert_eq!(json["format"], "email");
}

#[test]
fn integer_type() {
    let s = json_schema_to_schema(&json!({"type": "integer", "minimum": 0, "maximum": 100}));
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["type"], "integer");
    assert_eq!(json["minimum"], 0);
    assert_eq!(json["maximum"], 100);
}

#[test]
fn number_type_with_float() {
    let s = json_schema_to_schema(&json!({"type": "number", "minimum": 0.5, "maximum": 99.9}));
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["type"], "number");
    assert_eq!(json["minimum"], 0.5);
    assert_eq!(json["maximum"], 99.9);
}

#[test]
fn boolean_type() {
    let s = json_schema_to_schema(&json!({"type": "boolean"}));
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["type"], "boolean");
}

#[test]
fn null_type() {
    let s = json_schema_to_schema(&json!({"type": "null"}));
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["type"], "null");
}

#[test]
fn enum_values() {
    let s = json_schema_to_schema(&json!({"type": "string", "enum": ["a", "b", "c"]}));
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["enum"], json!(["a", "b", "c"]));
}

#[test]
fn object_with_properties() {
    let s = json_schema_to_schema(&json!({
        "type": "object",
        "required": ["name", "email"],
        "properties": {
            "name": {"type": "string", "minLength": 1},
            "email": {"type": "string", "format": "email"}
        }
    }));
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["type"], "object");
    assert_eq!(json["properties"]["name"]["type"], "string");
    assert_eq!(json["properties"]["email"]["format"], "email");
    let req = json["required"].as_array().unwrap();
    assert!(req.contains(&json!("name")));
    assert!(req.contains(&json!("email")));
}

#[test]
fn array_type() {
    let s = json_schema_to_schema(&json!({
        "type": "array",
        "items": {"type": "string"},
        "minItems": 1,
        "maxItems": 10
    }));
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["type"], "array");
    assert_eq!(json["items"]["type"], "string");
    assert_eq!(json["minItems"], 1);
    assert_eq!(json["maxItems"], 10);
}

#[test]
fn one_of() {
    let s = json_schema_to_schema(&json!({
        "oneOf": [
            {"type": "string"},
            {"type": "integer"}
        ]
    }));
    let json = serde_json::to_value(&s).unwrap();
    let items = json["oneOf"].as_array().unwrap();
    assert_eq!(items.len(), 2);
}

#[test]
fn all_of() {
    let s = json_schema_to_schema(&json!({
        "allOf": [
            {"type": "string", "minLength": 3},
            {"type": "string", "format": "email"}
        ]
    }));
    let json = serde_json::to_value(&s).unwrap();
    let items = json["allOf"].as_array().unwrap();
    assert_eq!(items.len(), 2);
}

#[test]
fn ref_schema() {
    let s = json_schema_to_schema(&json!({"$ref": "#/components/schemas/User"}));
    match s {
        RefOr::Ref(r) => assert_eq!(r.ref_location, "#/components/schemas/User"),
        _ => panic!("Expected Ref"),
    }
}

#[test]
fn description_and_default() {
    let s = json_schema_to_schema(&json!({
        "type": "string",
        "description": "User name",
        "default": "anonymous"
    }));
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["description"], "User name");
    assert_eq!(json["default"], "anonymous");
}

#[test]
fn exclusive_min_max() {
    let s = json_schema_to_schema(&json!({
        "type": "number",
        "exclusiveMinimum": 0,
        "exclusiveMaximum": 100
    }));
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["exclusiveMinimum"], 0);
    assert_eq!(json["exclusiveMaximum"], 100);
}

#[test]
fn multiple_of() {
    let s = json_schema_to_schema(&json!({"type": "integer", "multipleOf": 5}));
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["multipleOf"], 5);
}

#[test]
fn pattern() {
    let s = json_schema_to_schema(&json!({"type": "string", "pattern": "^[a-z]+$"}));
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["pattern"], "^[a-z]+$");
}

// ---- impl_to_schema! tests ----

vld::schema! {
    #[derive(Debug)]
    pub struct TestUser {
        pub name: String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
        pub age: Option<i64> => vld::number().int().gte(0).optional(),
    }
}

impl_to_schema!(TestUser);

#[test]
fn impl_to_schema_partial_schema() {
    let schema = TestUser::schema();
    let json = serde_json::to_value(&schema).unwrap();
    assert_eq!(json["type"], "object");
    assert!(json["properties"]["name"]["type"] == "string");
    assert!(json["properties"]["email"]["format"] == "email");
}

#[test]
fn impl_to_schema_name() {
    let name = TestUser::name();
    assert_eq!(name, "TestUser");
}

// Custom name
vld::schema! {
    #[derive(Debug)]
    pub struct ReqBody {
        pub x: String => vld::string().min(1),
    }
}

impl_to_schema!(ReqBody, "CreateRequest");

#[test]
fn impl_to_schema_custom_name() {
    let name = ReqBody::name();
    assert_eq!(name, "CreateRequest");
}

#[test]
fn impl_to_schema_custom_name_schema_works() {
    let schema = ReqBody::schema();
    let json = serde_json::to_value(&schema).unwrap();
    assert_eq!(json["type"], "object");
}

// ---- Round-trip: vld json_schema -> utoipa -> JSON ----

#[test]
fn roundtrip_string_schema() {
    let vld_schema = vld::string().min(3).max(100).email();
    use vld::json_schema::JsonSchema;
    let js = vld_schema.json_schema();

    let utoipa_schema = json_schema_to_schema(&js);
    let json = serde_json::to_value(&utoipa_schema).unwrap();

    assert_eq!(json["type"], "string");
    assert_eq!(json["minLength"], 3);
    assert_eq!(json["maxLength"], 100);
    assert_eq!(json["format"], "email");
}

#[test]
fn roundtrip_number_schema() {
    let vld_schema = vld::number().min(0.0).max(100.0);
    use vld::json_schema::JsonSchema;
    let js = vld_schema.json_schema();

    let utoipa_schema = json_schema_to_schema(&js);
    let json = serde_json::to_value(&utoipa_schema).unwrap();

    assert_eq!(json["type"], "number");
}

#[test]
fn roundtrip_array_schema() {
    let vld_schema = vld::array(vld::string().non_empty()).min_len(1);
    use vld::json_schema::JsonSchema;
    let js = vld_schema.json_schema();

    let utoipa_schema = json_schema_to_schema(&js);
    let json = serde_json::to_value(&utoipa_schema).unwrap();

    assert_eq!(json["type"], "array");
    assert_eq!(json["items"]["type"], "string");
    assert_eq!(json["minItems"], 1);
}

#[test]
fn roundtrip_optional_schema() {
    let vld_schema = vld::string().email().optional();
    use vld::json_schema::JsonSchema;
    let js = vld_schema.json_schema();

    let utoipa_schema = json_schema_to_schema(&js);
    let json = serde_json::to_value(&utoipa_schema).unwrap();

    // optional produces oneOf
    assert!(json["oneOf"].is_array());
}

#[test]
fn roundtrip_full_struct() {
    let js = TestUser::json_schema();
    let utoipa_schema = json_schema_to_schema(&js);
    let json = serde_json::to_value(&utoipa_schema).unwrap();

    assert_eq!(json["type"], "object");
    let req = json["required"].as_array().unwrap();
    assert!(req.contains(&json!("name")));
    assert!(req.contains(&json!("email")));
    assert!(req.contains(&json!("age")));
    assert_eq!(json["properties"]["name"]["minLength"], 2);
    assert_eq!(json["properties"]["name"]["maxLength"], 50);
    assert_eq!(json["properties"]["email"]["format"], "email");
}
