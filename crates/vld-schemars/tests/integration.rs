use serde_json::json;
use vld::openapi::JsonSchema;
use vld_schemars::schemars;
use vld_schemars::*;

mod common;
use common::{invalid, pairs, valid};

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
fn vld_to_schemars_invalid_fails_closed() {
    let schema = vld_to_schemars(&json!("not a schema"));
    assert_eq!(schema, schemars::Schema::from(false));
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
    let mut generator = schemars::SchemaGenerator::default();
    let schema = <UserSchema as schemars::JsonSchema>::json_schema(&mut generator);
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
    valid(&schema, &json!({"name": "Alice", "age": 30}));
    invalid(
        &schema,
        &json!({"name": "", "age": 151}),
        &[(".age", "too_big"), (".name", "too_small")], // properties in key order
    );
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
    invalid(&schema, &value, &[(".name", "missing_field")]);
}

#[test]
fn validate_with_schema_wrong_type() {
    invalid(
        &json!({"type": "string"}),
        &json!(42),
        &[("", "invalid_type")],
    );
}

#[test]
fn validate_with_schema_string_min_length() {
    let schema = json!({"type": "string", "minLength": 3});
    invalid(&schema, &json!("ab"), &[("", "too_small")]);
    valid(&schema, &json!("abc"));
}

#[test]
fn validate_with_schema_string_max_length() {
    let schema = json!({"type": "string", "maxLength": 3});
    invalid(&schema, &json!("abcd"), &[("", "too_big")]);
    valid(&schema, &json!("abc"));
}

#[test]
fn validate_with_schema_number_minimum() {
    let schema = json!({"type": "number", "minimum": 10});
    invalid(&schema, &json!(5), &[("", "too_small")]);
    valid(&schema, &json!(10));
    valid(&schema, &json!(15));
}

#[test]
fn validate_with_schema_number_maximum() {
    let schema = json!({"type": "number", "maximum": 100});
    invalid(&schema, &json!(150), &[("", "too_big")]);
    valid(&schema, &json!(100));
}

#[test]
fn validate_with_schema_exclusive_min_max() {
    let schema = json!({"type": "number", "exclusiveMinimum": 0, "exclusiveMaximum": 10});
    invalid(&schema, &json!(0), &[("", "too_small")]);
    invalid(&schema, &json!(10), &[("", "too_big")]);
    valid(&schema, &json!(5));
}

#[test]
fn validate_with_schema_string_pattern() {
    let schema = json!({"type": "string", "pattern": "^[a-z]+$"});
    valid(&schema, &json!("hello"));
    invalid(&schema, &json!("Hello123"), &[("", "invalid_string")]);
}

#[test]
fn validate_with_schema_string_format_email() {
    let schema = json!({"type": "string", "format": "email"});
    valid(&schema, &json!("user@example.com"));
    invalid(&schema, &json!("not-email"), &[("", "invalid_string")]);
}

#[test]
fn validate_with_schema_enum() {
    let schema = json!({"enum": ["red", "green", "blue"]});
    valid(&schema, &json!("red"));
    invalid(&schema, &json!("yellow"), &[("", "enum")]);
}

#[test]
fn validate_with_schema_array_min_items() {
    let schema = json!({"type": "array", "minItems": 2});
    invalid(&schema, &json!([1]), &[("", "too_small")]);
    valid(&schema, &json!([1, 2]));
}

#[test]
fn validate_with_schema_array_items() {
    let schema = json!({
        "type": "array",
        "items": {"type": "integer", "minimum": 0}
    });
    valid(&schema, &json!([1, 2, 3]));
    invalid(&schema, &json!([1, -1, 3]), &[("[1]", "too_small")]);
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
    valid(&schema, &json!({"address": {"city": "NYC"}}));
    invalid(
        &schema,
        &json!({"address": {"city": ""}}),
        &[(".address.city", "too_small")],
    );
    invalid(
        &schema,
        &json!({"address": {}}),
        &[(".address.city", "missing_field")],
    );
}

#[test]
fn validate_with_schema_boolean_schema() {
    valid(&json!(true), &json!("anything"));
    invalid(&json!(false), &json!("anything"), &[("", "false_schema")]);
}

#[test]
fn validate_with_schema_any_of() {
    let schema = json!({
        "anyOf": [
            {"type": "string"},
            {"type": "integer"}
        ]
    });
    valid(&schema, &json!("hello"));
    valid(&schema, &json!(42));
    invalid(&schema, &json!(true), &[("", "anyOf")]);
}

#[test]
fn validate_with_schema_all_of() {
    let schema = json!({
        "allOf": [
            {"type": "number", "minimum": 0},
            {"type": "number", "maximum": 100}
        ]
    });
    valid(&schema, &json!(50));
    invalid(&schema, &json!(150), &[("", "too_big")]);
    invalid(&schema, &json!(-1), &[("", "too_small")]);
}

#[test]
fn validate_with_schema_not() {
    let schema = json!({
        "not": {"type": "string"}
    });
    valid(&schema, &json!(42));
    invalid(&schema, &json!("hello"), &[("", "not")]);
}

// ========================= validate_with_schemars ============================

#[test]
fn validate_with_schemars_valid() {
    let schema = vld_to_schemars(&json!({"type": "string", "minLength": 2}));
    vld_schemars::validate_with_schemars(&schema, &json!("hello")).unwrap();
}

#[test]
fn validate_with_schemars_invalid() {
    let schema = vld_to_schemars(&json!({"type": "string", "minLength": 5}));
    let err = vld_schemars::validate_with_schemars(&schema, &json!("hi")).unwrap_err();
    assert_eq!(pairs(&err), [("".to_string(), "too_small".to_string())]);
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
    let err = Item::vld_parse_value(&json).unwrap_err();
    assert_eq!(
        pairs(&err),
        [(".qty".to_string(), "missing_field".to_string())]
    );
}

#[test]
fn impl_vld_parse_wrong_type() {
    use vld::schema::VldParse;
    let json = json!({"name": 123, "qty": "not a number"});
    let err = Item::vld_parse_value(&json).unwrap_err();
    let got = pairs(&err);
    assert!(
        got.contains(&(".name".into(), "invalid_type".into())),
        "{got:?}"
    );
    assert!(
        got.contains(&(".qty".into(), "invalid_type".into())),
        "{got:?}"
    );
}

// --- SchemarsValidate::vld_validate (validate existing instance) ---

#[test]
fn schemars_validate_valid_instance() {
    use vld_schemars::SchemarsValidate;
    let user = User {
        name: "Alice".into(),
        age: 30,
    };
    user.vld_validate().unwrap();
}

#[test]
fn schemars_validate_item() {
    use vld_schemars::SchemarsValidate;
    let item = Item {
        name: "Widget".into(),
        qty: 5,
    };
    item.vld_validate().unwrap();
}

// --- SchemarsValidate::vld_validate_json (validate JSON against type's schema) ---

#[test]
fn schemars_validate_json_valid() {
    use vld_schemars::SchemarsValidate;
    let json = json!({"name": "Alice", "age": 30});
    User::vld_validate_json(&json).unwrap();
}

#[test]
fn schemars_validate_json_missing_field() {
    use vld_schemars::SchemarsValidate;
    let json = json!({"name": "Alice"});
    let err = User::vld_validate_json(&json).unwrap_err();
    assert_eq!(
        pairs(&err),
        [(".age".to_string(), "missing_field".to_string())]
    );
}

#[test]
fn schemars_validate_json_wrong_type() {
    use vld_schemars::SchemarsValidate;
    let json = json!({"name": 123, "age": "not a number"});
    let err = User::vld_validate_json(&json).unwrap_err();
    let got = pairs(&err);
    assert!(
        got.contains(&(".name".into(), "invalid_type".into())),
        "{got:?}"
    );
    assert!(
        got.contains(&(".age".into(), "invalid_type".into())),
        "{got:?}"
    );
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
    match User::vld_parse(&json) {
        Err(VldSchemarsError::Validation(err)) => {
            assert_eq!(
                pairs(&err),
                [(".age".to_string(), "missing_field".to_string())]
            );
        }
        other => panic!("expected a validation error, got {other:?}"),
    }
}

#[test]
fn schemars_vld_parse_item() {
    use vld_schemars::SchemarsValidate;
    let json = json!({"name": "Gadget", "qty": 10});
    let item = Item::vld_parse(&json).unwrap();
    assert_eq!(item.name, "Gadget");
    assert_eq!(item.qty, 10);
}

// ========================= Nested schema registration ========================

vld::schema! {
    #[derive(Debug)]
    pub struct SchemarsAddr {
        pub city: String => vld::string().min(1),
        pub zip: String  => vld::string().min(5).max(10),
    }
}

impl_json_schema!(SchemarsAddr);

vld::schema! {
    #[derive(Debug)]
    pub struct SchemarsCompany {
        pub name: String => vld::string().min(1),
        pub hq: SchemarsAddr => vld::nested!(SchemarsAddr),
    }
}

impl_json_schema!(SchemarsCompany);

#[test]
fn nested_schema_registered_in_generator_definitions() {
    let mut generator = schemars::SchemaGenerator::default();
    let schema = <SchemarsCompany as schemars::JsonSchema>::json_schema(&mut generator);
    let props = schema.get("properties").unwrap();
    // Refs follow the generator's definitions path (default `$defs`).
    assert_eq!(props["hq"]["$ref"], "#/$defs/SchemarsAddr");

    let defs = generator.definitions();
    assert!(
        defs.contains_key("SchemarsAddr"),
        "SchemarsAddr should be in definitions, got: {:?}",
        defs.keys().collect::<Vec<_>>()
    );
    let addr = &defs["SchemarsAddr"];
    assert_eq!(addr["type"], "object");
    assert!(addr["properties"]["city"].is_object());
    assert!(addr["properties"]["zip"].is_object());
}

/// Regression for https://github.com/s00d/vld/issues/5 — grandchild schemas
/// must appear in schemars definitions transitively.
#[test]
fn transitive_nested_schema_registered_in_definitions() {
    vld::schema! {
        #[derive(Debug)]
        pub struct SchemarsBreak {
            pub label: String => vld::string().min(1),
        }
    }
    impl_json_schema!(SchemarsBreak);

    vld::schema! {
        #[derive(Debug)]
        pub struct SchemarsDay {
            pub breaks: Option<Vec<SchemarsBreak>> =>
                vld::array(vld::nested!(SchemarsBreak)).optional(),
        }
    }
    impl_json_schema!(SchemarsDay);

    vld::schema! {
        #[derive(Debug)]
        pub struct SchemarsSchedule {
            pub days: Vec<SchemarsDay> => vld::array(vld::nested!(SchemarsDay)),
        }
    }
    impl_json_schema!(SchemarsSchedule);

    let mut generator = schemars::SchemaGenerator::default();
    let _schema = <SchemarsSchedule as schemars::JsonSchema>::json_schema(&mut generator);
    let defs = generator.definitions();

    assert!(
        defs.contains_key("SchemarsDay"),
        "child SchemarsDay missing: {:?}",
        defs.keys().collect::<Vec<_>>()
    );
    assert!(
        defs.contains_key("SchemarsBreak"),
        "grandchild SchemarsBreak missing: {:?}",
        defs.keys().collect::<Vec<_>>()
    );
}
