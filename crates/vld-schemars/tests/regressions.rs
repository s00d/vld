use serde_json::json;
use vld_schemars::schemars;
use vld_schemars::*;

vld::schema! {
    #[derive(Debug)]
    pub struct RegAddr {
        pub city: String => vld::string().min(1),
        pub zip: String  => vld::string().min(5).max(10),
    }
}
impl_json_schema!(RegAddr);

vld::schema! {
    #[derive(Debug)]
    pub struct RegCompany {
        pub name: String => vld::string().min(1),
        pub hq: RegAddr => vld::nested!(RegAddr),
    }
}
impl_json_schema!(RegCompany);

#[test]
fn standalone_root_schema_has_resolvable_refs() {
    let root = generate_from_schemars::<RegCompany>();
    let r = root["properties"]["hq"]["$ref"].as_str().unwrap();
    let ptr = r.strip_prefix('#').unwrap();
    assert!(root.pointer(ptr).is_some(), "dangling ref {r} in {root}");
    // and the validator follows it
    let ok = json!({"name": "Acme", "hq": {"city": "Paris", "zip": "75001"}});
    let bad = json!({"name": "Acme", "hq": {"city": "Paris", "zip": "1"}});
    assert!(validate_with_schema(&root, &ok).is_ok());
    assert!(validate_with_schema(&root, &bad).is_err());
}

#[test]
fn openapi3_generator_keeps_components_path() {
    let mut gen = schemars::generate::SchemaSettings::openapi3().into_generator();
    let schema = <RegCompany as schemars::JsonSchema>::json_schema(&mut gen);
    assert_eq!(
        schema.get("properties").unwrap()["hq"]["$ref"],
        "#/components/schemas/RegAddr"
    );
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct Inner {
    #[schemars(length(min = 3))]
    code: String,
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct Outer {
    inner: Inner,
    pair: (u8, String),
}

#[test]
fn validator_follows_ref_to_nested_struct() {
    let schema = generate_from_schemars::<Outer>();
    let ok = json!({"inner": {"code": "abc"}, "pair": [1, "x"]});
    let bad = json!({"inner": {"code": "a"}, "pair": [1, "x"]});
    assert!(validate_with_schema(&schema, &ok).is_ok());
    let err = validate_with_schema(&schema, &bad).unwrap_err();
    assert_eq!(err.issues[0].path.len(), 2, "{err:?}");
}

#[test]
fn validator_enforces_additional_properties_false() {
    let schema = generate_from_schemars::<Outer>();
    let extra = json!({"inner": {"code": "abc"}, "pair": [1, "x"], "extra": 1});
    let err = validate_with_schema(&schema, &extra).unwrap_err();
    assert!(matches!(
        err.issues[0].code,
        vld::error::IssueCode::UnrecognizedField
    ));
}

#[test]
fn validator_additional_properties_schema() {
    let schema = json!({"type": "object", "additionalProperties": {"type": "integer"}});
    assert!(validate_with_schema(&schema, &json!({"a": 1})).is_ok());
    assert!(validate_with_schema(&schema, &json!({"a": "x"})).is_err());
}

#[test]
fn validator_checks_prefix_items() {
    let schema = generate_from_schemars::<Outer>();
    let bad = json!({"inner": {"code": "abc"}, "pair": ["no", "x"]});
    assert!(validate_with_schema(&schema, &bad).is_err());
    let raw = json!({"prefixItems": [{"type": "integer"}], "items": {"type": "string"}});
    assert!(validate_with_schema(&raw, &json!([1, "a", "b"])).is_ok());
    assert!(validate_with_schema(&raw, &json!([1, 2])).is_err());
}

#[test]
fn validator_string_length_counts_chars() {
    let schema = json!({"type": "string", "maxLength": 3, "minLength": 3});
    assert!(validate_with_schema(&schema, &json!("äöü")).is_ok());
}

#[test]
fn validator_multiple_of_float_tolerance() {
    let schema = json!({"type": "number", "multipleOf": 0.1});
    assert!(validate_with_schema(&schema, &json!(0.3)).is_ok());
    assert!(validate_with_schema(&schema, &json!(0.35)).is_err());
}

#[test]
fn validator_invalid_pattern_is_error() {
    let schema = json!({"type": "string", "pattern": "(unclosed"});
    let err = validate_with_schema(&schema, &json!("x")).unwrap_err();
    assert!(err.issues[0].message.contains("Invalid pattern"));
}

#[test]
fn validator_uri_accepts_any_scheme() {
    let schema = json!({"type": "string", "format": "uri"});
    for ok in ["mailto:a@b.c", "urn:isbn:123", "ftp://x.y", "https://x.y"] {
        assert!(validate_with_schema(&schema, &json!(ok)).is_ok(), "{ok}");
    }
    for bad in ["no-scheme", "1http://x", "http://a b"] {
        assert!(validate_with_schema(&schema, &json!(bad)).is_err(), "{bad}");
    }
}

#[test]
fn validator_any_of_union() {
    let schema = json!({"anyOf": [{"type": "string"}, {"type": "null"}]});
    assert!(validate_with_schema(&schema, &json!(null)).is_ok());
    assert!(validate_with_schema(&schema, &json!(1)).is_err());
}

#[test]
fn validator_self_ref_does_not_overflow() {
    assert!(validate_with_schema(&json!({"$ref": "#"}), &json!(1)).is_err());
    let dangling = json!({"$ref": "#/$defs/Missing"});
    assert!(validate_with_schema(&dangling, &json!(1)).is_err());
}
