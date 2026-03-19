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
    let base =
        json!({"type": "object", "properties": {"name": {"type": "string", "minLength": 5}}});
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

// ========================= schemars → vld validation =========================

#[test]
fn validate_with_schema_valid_object() {
    let schema = json!({
        "type": "object",
        "required": ["name", "age"],
        "properties": {
            "name": { "type": "string", "minLength": 1, "maxLength": 50 },
            "age":  { "type": "integer", "minimum": 0, "maximum": 150 }
        }
    });
    let value = json!({"name": "Alice", "age": 30});
    assert!(vld_schemars::validate_with_schema(&schema, &value).is_ok());
}

#[test]
fn validate_with_schema_missing_required() {
    let schema = json!({
        "type": "object",
        "required": ["name"],
        "properties": { "name": {"type": "string"} }
    });
    let value = json!({});
    let err = vld_schemars::validate_with_schema(&schema, &value).unwrap_err();
    assert!(err.issues.iter().any(|i| i.message.contains("name")));
}

#[test]
fn validate_with_schema_wrong_type() {
    let schema = json!({"type": "string"});
    let value = json!(42);
    assert!(vld_schemars::validate_with_schema(&schema, &value).is_err());
}

#[test]
fn validate_with_schema_string_min_length() {
    let schema = json!({"type": "string", "minLength": 3});
    assert!(vld_schemars::validate_with_schema(&schema, &json!("ab")).is_err());
    assert!(vld_schemars::validate_with_schema(&schema, &json!("abc")).is_ok());
}

#[test]
fn validate_with_schema_string_max_length() {
    let schema = json!({"type": "string", "maxLength": 3});
    assert!(vld_schemars::validate_with_schema(&schema, &json!("abcd")).is_err());
    assert!(vld_schemars::validate_with_schema(&schema, &json!("abc")).is_ok());
}

#[test]
fn validate_with_schema_number_minimum() {
    let schema = json!({"type": "number", "minimum": 10});
    assert!(vld_schemars::validate_with_schema(&schema, &json!(5)).is_err());
    assert!(vld_schemars::validate_with_schema(&schema, &json!(10)).is_ok());
    assert!(vld_schemars::validate_with_schema(&schema, &json!(15)).is_ok());
}

#[test]
fn validate_with_schema_number_maximum() {
    let schema = json!({"type": "number", "maximum": 100});
    assert!(vld_schemars::validate_with_schema(&schema, &json!(150)).is_err());
    assert!(vld_schemars::validate_with_schema(&schema, &json!(100)).is_ok());
}

#[test]
fn validate_with_schema_exclusive_min_max() {
    let schema = json!({"type": "number", "exclusiveMinimum": 0, "exclusiveMaximum": 10});
    assert!(vld_schemars::validate_with_schema(&schema, &json!(0)).is_err());
    assert!(vld_schemars::validate_with_schema(&schema, &json!(10)).is_err());
    assert!(vld_schemars::validate_with_schema(&schema, &json!(5)).is_ok());
}

#[test]
fn validate_with_schema_string_pattern() {
    let schema = json!({"type": "string", "pattern": "^[a-z]+$"});
    assert!(vld_schemars::validate_with_schema(&schema, &json!("hello")).is_ok());
    assert!(vld_schemars::validate_with_schema(&schema, &json!("Hello123")).is_err());
}

#[test]
fn validate_with_schema_string_format_email() {
    let schema = json!({"type": "string", "format": "email"});
    assert!(vld_schemars::validate_with_schema(&schema, &json!("user@example.com")).is_ok());
    assert!(vld_schemars::validate_with_schema(&schema, &json!("not-email")).is_err());
}

#[test]
fn validate_with_schema_enum() {
    let schema = json!({"enum": ["red", "green", "blue"]});
    assert!(vld_schemars::validate_with_schema(&schema, &json!("red")).is_ok());
    assert!(vld_schemars::validate_with_schema(&schema, &json!("yellow")).is_err());
}

#[test]
fn validate_with_schema_array_min_items() {
    let schema = json!({"type": "array", "minItems": 2});
    assert!(vld_schemars::validate_with_schema(&schema, &json!([1])).is_err());
    assert!(vld_schemars::validate_with_schema(&schema, &json!([1, 2])).is_ok());
}

#[test]
fn validate_with_schema_array_items() {
    let schema = json!({
        "type": "array",
        "items": {"type": "integer", "minimum": 0}
    });
    assert!(vld_schemars::validate_with_schema(&schema, &json!([1, 2, 3])).is_ok());
    assert!(vld_schemars::validate_with_schema(&schema, &json!([1, -1, 3])).is_err());
}

#[test]
fn validate_with_schema_nested_object() {
    let schema = json!({
        "type": "object",
        "required": ["address"],
        "properties": {
            "address": {
                "type": "object",
                "required": ["city"],
                "properties": {
                    "city": {"type": "string", "minLength": 1}
                }
            }
        }
    });
    assert!(
        vld_schemars::validate_with_schema(&schema, &json!({"address": {"city": "NYC"}})).is_ok()
    );
    assert!(
        vld_schemars::validate_with_schema(&schema, &json!({"address": {"city": ""}})).is_err()
    );
    assert!(vld_schemars::validate_with_schema(&schema, &json!({"address": {}})).is_err());
}

#[test]
fn validate_with_schema_boolean_schema() {
    assert!(vld_schemars::validate_with_schema(&json!(true), &json!("anything")).is_ok());
    assert!(vld_schemars::validate_with_schema(&json!(false), &json!("anything")).is_err());
}

#[test]
fn validate_with_schema_any_of() {
    let schema = json!({
        "anyOf": [
            {"type": "string"},
            {"type": "integer"}
        ]
    });
    assert!(vld_schemars::validate_with_schema(&schema, &json!("hello")).is_ok());
    assert!(vld_schemars::validate_with_schema(&schema, &json!(42)).is_ok());
    assert!(vld_schemars::validate_with_schema(&schema, &json!(true)).is_err());
}

#[test]
fn validate_with_schema_all_of() {
    let schema = json!({
        "allOf": [
            {"type": "number", "minimum": 0},
            {"type": "number", "maximum": 100}
        ]
    });
    assert!(vld_schemars::validate_with_schema(&schema, &json!(50)).is_ok());
    assert!(vld_schemars::validate_with_schema(&schema, &json!(150)).is_err());
}

#[test]
fn validate_with_schema_not() {
    let schema = json!({
        "not": {"type": "string"}
    });
    assert!(vld_schemars::validate_with_schema(&schema, &json!(42)).is_ok());
    assert!(vld_schemars::validate_with_schema(&schema, &json!("hello")).is_err());
}

// ========================= validate_with_schemars ============================

#[test]
fn validate_with_schemars_valid() {
    let schema = vld_to_schemars(&json!({"type": "string", "minLength": 2}));
    assert!(vld_schemars::validate_with_schemars(&schema, &json!("hello")).is_ok());
}

#[test]
fn validate_with_schemars_invalid() {
    let schema = vld_to_schemars(&json!({"type": "string", "minLength": 5}));
    assert!(vld_schemars::validate_with_schemars(&schema, &json!("hi")).is_err());
}

// ========================= impl_vld_parse! + SchemarsValidate ================

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct User {
    name: String,
    age: u32,
}

vld_schemars::impl_vld_parse!(User);

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct Item {
    name: String,
    qty: u32,
}

vld_schemars::impl_vld_parse!(Item);

// --- VldParse (parse from JSON) ---

#[test]
fn impl_vld_parse_valid() {
    use vld::schema::VldParse;
    let json = json!({"name": "Widget", "qty": 5});
    let item = Item::vld_parse_value(&json).unwrap();
    assert_eq!(item.name, "Widget");
    assert_eq!(item.qty, 5);
}

#[test]
fn impl_vld_parse_missing_field() {
    use vld::schema::VldParse;
    let json = json!({"name": "Widget"});
    assert!(Item::vld_parse_value(&json).is_err());
}

#[test]
fn impl_vld_parse_wrong_type() {
    use vld::schema::VldParse;
    let json = json!({"name": 123, "qty": "not a number"});
    assert!(Item::vld_parse_value(&json).is_err());
}

// --- SchemarsValidate::vld_validate (validate existing instance) ---

#[test]
fn schemars_validate_valid_instance() {
    use vld_schemars::SchemarsValidate;
    let user = User {
        name: "Alice".into(),
        age: 30,
    };
    assert!(user.vld_validate().is_ok());
}

#[test]
fn schemars_validate_item() {
    use vld_schemars::SchemarsValidate;
    let item = Item {
        name: "Widget".into(),
        qty: 5,
    };
    assert!(item.vld_validate().is_ok());
}

// --- SchemarsValidate::vld_validate_json (validate JSON against type's schema) ---

#[test]
fn schemars_validate_json_valid() {
    use vld_schemars::SchemarsValidate;
    let json = json!({"name": "Alice", "age": 30});
    assert!(User::vld_validate_json(&json).is_ok());
}

#[test]
fn schemars_validate_json_missing_field() {
    use vld_schemars::SchemarsValidate;
    let json = json!({"name": "Alice"});
    assert!(User::vld_validate_json(&json).is_err());
}

#[test]
fn schemars_validate_json_wrong_type() {
    use vld_schemars::SchemarsValidate;
    let json = json!({"name": 123, "age": "not a number"});
    assert!(User::vld_validate_json(&json).is_err());
}

// --- SchemarsValidate::vld_parse (validate + deserialize) ---

#[test]
fn schemars_vld_parse_valid() {
    use vld_schemars::SchemarsValidate;
    let json = json!({"name": "Bob", "age": 25});
    let user = User::vld_parse(&json).unwrap();
    assert_eq!(user.name, "Bob");
    assert_eq!(user.age, 25);
}

#[test]
fn schemars_vld_parse_invalid() {
    use vld_schemars::SchemarsValidate;
    let json = json!({"name": "Bob"});
    assert!(User::vld_parse(&json).is_err());
}

#[test]
fn schemars_vld_parse_item() {
    use vld_schemars::SchemarsValidate;
    let json = json!({"name": "Gadget", "qty": 10});
    let item = Item::vld_parse(&json).unwrap();
    assert_eq!(item.name, "Gadget");
    assert_eq!(item.qty, 10);
}
