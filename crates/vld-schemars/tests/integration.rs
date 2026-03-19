use serde_json::json;
use vld::json_schema::JsonSchema;
use vld_schemars::*;

// ========================= Schemas ===========================================

vld::schema! {
    #[derive(Debug)]
    pub struct UserSchema {
        pub name: String  => vld::string().min(2).max(100),
        pub email: String => vld::string().email(),
        pub age: i64      => vld::number().int().min(0).max(150),
    }
}

impl_json_schema!(UserSchema);

vld::schema! {
    #[derive(Debug)]
    pub struct SimpleSchema {
        pub title: String => vld::string().min(1),
    }
}

impl_json_schema!(SimpleSchema, "SimpleItem");

// ========================= vld → schemars ====================================

#[test]
fn vld_to_schemars_object() {
    let vld_json = UserSchema::json_schema();
    let schemars_schema = vld_to_schemars(&vld_json);
    assert_eq!(schemars_schema.get("type").unwrap(), "object");
    assert!(schemars_schema.get("properties").is_some());
}

#[test]
fn vld_to_schemars_primitive() {
    let vld_json = vld::string().email().json_schema();
    let schemars_schema = vld_to_schemars(&vld_json);
    assert_eq!(schemars_schema.get("type").unwrap(), "string");
    assert_eq!(schemars_schema.get("format").unwrap(), "email");
}

#[test]
fn vld_to_schemars_invalid_falls_back() {
    let schema = vld_to_schemars(&json!("not a schema"));
    assert_eq!(schema, schemars::Schema::default());
}

#[test]
fn vld_schema_to_schemars_works() {
    let vld_json = vld::number().int().min(0).json_schema();
    let schema = vld_schema_to_schemars(&vld_json);
    assert_eq!(schema.get("type").unwrap(), "integer");
}

// ========================= schemars → vld ====================================

#[test]
fn schemars_to_json_roundtrip() {
    let original = json!({"type": "string", "minLength": 1});
    let schemars_schema = vld_to_schemars(&original);
    let back = schemars_to_json(&schemars_schema);
    assert_eq!(original, back);
}

#[test]
fn generate_from_schemars_string() {
    let schema = generate_from_schemars::<String>();
    assert!(schema.is_object());
    // Root schema from schemars includes $schema, type, etc.
    assert!(schema.get("type").is_some() || schema.get("$schema").is_some());
}

#[test]
fn generate_from_schemars_i32() {
    let schema = generate_from_schemars::<i32>();
    assert!(schema.is_object());
}

#[test]
fn generate_from_schemars_bool() {
    let schema = generate_from_schemars::<bool>();
    assert!(schema.is_object());
}

#[test]
fn generate_schemars_returns_schema_type() {
    let schema = generate_schemars::<String>();
    assert!(schema.as_value().is_object());
}

// ========================= impl_json_schema! =================================

#[test]
fn impl_json_schema_basic() {
    let mut gen = schemars::SchemaGenerator::default();
    let schema = <UserSchema as schemars::JsonSchema>::json_schema(&mut gen);
    assert_eq!(schema.get("type").unwrap(), "object");

    let props = schema.get("properties").unwrap().as_object().unwrap();
    assert!(props.contains_key("name"));
    assert!(props.contains_key("email"));
    assert!(props.contains_key("age"));
}

#[test]
fn impl_json_schema_name() {
    let name = <UserSchema as schemars::JsonSchema>::schema_name();
    assert_eq!(&*name, "UserSchema");
}

#[test]
fn impl_json_schema_custom_name() {
    let name = <SimpleSchema as schemars::JsonSchema>::schema_name();
    assert_eq!(&*name, "SimpleItem");
}

#[test]
fn impl_json_schema_id_contains_module() {
    let id = <UserSchema as schemars::JsonSchema>::schema_id();
    assert!(id.contains("UserSchema"));
}

#[test]
fn impl_json_schema_custom_id() {
    let id = <SimpleSchema as schemars::JsonSchema>::schema_id();
    assert!(id.contains("SimpleItem"));
}

// ========================= Introspection =====================================

#[test]
fn list_properties_from_vld() {
    let vld_json = UserSchema::json_schema();
    let props = list_properties(&vld_json);
    assert_eq!(props.len(), 3);

    let names: Vec<&str> = props.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"name"));
    assert!(names.contains(&"email"));
    assert!(names.contains(&"age"));
}

#[test]
fn list_properties_required() {
    let vld_json = UserSchema::json_schema();
    let props = list_properties(&vld_json);

    for p in &props {
        assert!(p.required, "field '{}' should be required", p.name);
    }
}

#[test]
fn list_properties_schemars_wrapper() {
    let schemars_schema = vld_to_schemars(&UserSchema::json_schema());
    let props = list_properties_schemars(&schemars_schema);
    assert_eq!(props.len(), 3);
}

#[test]
fn list_properties_empty_schema() {
    let schema = json!({"type": "string"});
    let props = list_properties(&schema);
    assert!(props.is_empty());
}

#[test]
fn schema_type_string() {
    let schema = json!({"type": "string"});
    assert_eq!(schema_type(&schema), Some("string".to_string()));
}

#[test]
fn schema_type_object() {
    let schema = json!({"type": "object"});
    assert_eq!(schema_type(&schema), Some("object".to_string()));
}

#[test]
fn schema_type_missing() {
    let schema = json!({});
    assert_eq!(schema_type(&schema), None);
}

#[test]
fn is_required_true() {
    let schema = json!({
        "type": "object",
        "required": ["name", "email"],
        "properties": { "name": {"type": "string"}, "email": {"type": "string"} }
    });
    assert!(is_required(&schema, "name"));
    assert!(is_required(&schema, "email"));
}

#[test]
fn is_required_false() {
    let schema = json!({
        "type": "object",
        "required": ["name"],
        "properties": { "name": {"type": "string"}, "bio": {"type": "string"} }
    });
    assert!(!is_required(&schema, "bio"));
    assert!(!is_required(&schema, "nonexistent"));
}

#[test]
fn get_property_existing() {
    let schema = json!({
        "type": "object",
        "properties": { "name": {"type": "string", "minLength": 2} }
    });
    let prop = get_property(&schema, "name").unwrap();
    assert_eq!(prop["type"], "string");
    assert_eq!(prop["minLength"], 2);
}

#[test]
fn get_property_missing() {
    let schema = json!({"type": "object", "properties": {}});
    assert!(get_property(&schema, "nonexistent").is_none());
}

// ========================= Comparison & Merge ================================

#[test]
fn schemas_equal_same() {
    let a = json!({"type": "string", "minLength": 1});
    let b = json!({"type": "string", "minLength": 1});
    assert!(schemas_equal(&a, &b));
}

#[test]
fn schemas_equal_different() {
    let a = json!({"type": "string"});
    let b = json!({"type": "integer"});
    assert!(!schemas_equal(&a, &b));
}

#[test]
fn merge_schemas_creates_allof() {
    let a = vld_to_schemars(&json!({"type": "object", "properties": {"x": {"type": "string"}}}));
    let b = vld_to_schemars(&json!({"type": "object", "properties": {"y": {"type": "integer"}}}));
    let merged = merge_schemas(&a, &b);
    let all_of = merged.get("allOf").unwrap().as_array().unwrap();
    assert_eq!(all_of.len(), 2);
}

#[test]
fn overlay_constraints_adds_required() {
    let base = json!({"type": "object", "properties": {"name": {"type": "string"}}});
    let overlay = json!({"required": ["name"]});
    let result = overlay_constraints(&base, &overlay);
    assert!(is_required(&result, "name"));
}

#[test]
fn overlay_constraints_merges_properties() {
    let base = json!({"type": "object", "properties": {"name": {"type": "string"}}});
    let overlay = json!({"properties": {"name": {"minLength": 2}}});
    let result = overlay_constraints(&base, &overlay);
    let name_prop = get_property(&result, "name").unwrap();
    assert_eq!(name_prop["type"], "string");
    assert_eq!(name_prop["minLength"], 2);
}

#[test]
fn overlay_constraints_adds_new_property() {
    let base = json!({"type": "object", "properties": {"name": {"type": "string"}}});
    let overlay = json!({"properties": {"age": {"type": "integer"}}});
    let result = overlay_constraints(&base, &overlay);
    assert!(get_property(&result, "age").is_some());
}

#[test]
fn overlay_preserves_base_values() {
    let base = json!({"type": "object", "properties": {"name": {"type": "string", "minLength": 5}}});
    let overlay = json!({"properties": {"name": {"minLength": 2, "maxLength": 100}}});
    let result = overlay_constraints(&base, &overlay);
    let name_prop = get_property(&result, "name").unwrap();
    // base value is preserved (5, not overwritten by 2)
    assert_eq!(name_prop["minLength"], 5);
    // new value from overlay is added
    assert_eq!(name_prop["maxLength"], 100);
}

// ========================= Roundtrip tests ===================================

#[test]
fn vld_schemars_roundtrip() {
    let vld_json = UserSchema::json_schema();
    let schemars_schema = vld_to_schemars(&vld_json);
    let back = schemars_to_json(&schemars_schema);
    assert_eq!(vld_json, back);
}

#[test]
fn property_info_types() {
    let vld_json = UserSchema::json_schema();
    let props = list_properties(&vld_json);

    let name_prop = props.iter().find(|p| p.name == "name").unwrap();
    assert_eq!(name_prop.schema_type.as_deref(), Some("string"));

    let age_prop = props.iter().find(|p| p.name == "age").unwrap();
    assert_eq!(age_prop.schema_type.as_deref(), Some("integer"));
}

// ========================= derive(Validate) with schemars ====================

