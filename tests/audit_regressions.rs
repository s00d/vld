//! Regression tests for the 0.5.0 audit fixes (core, macros, derive, openapi).
#![allow(dead_code)]

use serde_json::json;
use vld::prelude::*;

// ---------------------------------------------------------------------------
// error.rs: truncate_value must not panic on multi-byte strings
// ---------------------------------------------------------------------------

#[test]
fn received_value_truncation_is_utf8_safe() {
    // 51 chars / 102 bytes: byte slicing at 97 used to split a char and panic.
    let s = "é".repeat(51);
    let err = vld::string().max(1).parse_value(&json!(s)).unwrap_err();
    assert_eq!(err.issues[0].received, Some(json!(s)));

    let long = "é".repeat(150);
    let err = vld::string().max(1).parse_value(&json!(long)).unwrap_err();
    let got = err.issues[0].received.as_ref().unwrap().as_str().unwrap();
    assert_eq!(got.chars().count(), 100);
    assert!(got.ends_with("..."));
}

// ---------------------------------------------------------------------------
// object.rs: partial()/field_optional keep defaults and catch
// ---------------------------------------------------------------------------

#[test]
fn partial_keeps_inner_default_and_catch() {
    let schema = vld::object()
        .field("n", vld::number().with_default(5.0))
        .field("s", vld::string().min(3).catch("anon".to_string()))
        .field("plain", vld::string())
        .partial();
    let out = schema.parse_value(&json!({})).unwrap();
    assert_eq!(out["n"], json!(5.0));
    assert_eq!(out["s"], json!("anon"));
    assert_eq!(out["plain"], json!(null));
}

#[test]
fn field_optional_keeps_default() {
    let schema = vld::object().field_optional("n", vld::number().with_default(1.0));
    assert_eq!(schema.parse_value(&json!({})).unwrap()["n"], json!(1.0));
    assert_eq!(
        schema.parse_value(&json!({"n": 2})).unwrap()["n"],
        json!(2.0)
    );
}

// ---------------------------------------------------------------------------
// macros.rs
// ---------------------------------------------------------------------------

vld::schema! {
    #[derive(Debug)]
    pub struct RawIdent {
        pub r#type: String => vld::string(),
    }
}

#[test]
fn schema_macro_strips_raw_identifier_prefix() {
    let v = RawIdent::parse_value(&json!({"type": "a"})).unwrap();
    assert_eq!(v.r#type, "a");
    let err = RawIdent::parse_value(&json!({})).unwrap_err();
    assert_eq!(err.issues[0].path[0].to_string(), ".type");
}

#[test]
fn union_macro_supports_seven_or_more() {
    let s = vld::union!(
        vld::literal("a"),
        vld::literal("b"),
        vld::literal("c"),
        vld::literal("d"),
        vld::literal("e"),
        vld::literal("f"),
        vld::literal("g"),
        vld::literal("h"),
    );
    assert!(s.parse_value(&json!("h")).is_ok());
    assert!(s.parse_value(&json!("z")).is_err());
}

struct Wire {
    first_name: String,
}

vld::impl_rules!(Wire {
    first_name as "firstName" => vld::string().min(2),
});

#[test]
fn impl_rules_supports_rename() {
    let err = Wire {
        first_name: "x".into(),
    }
    .validate()
    .unwrap_err();
    assert_eq!(err.issues[0].path[0].to_string(), ".firstName");
}

// ---------------------------------------------------------------------------
// openapi
// ---------------------------------------------------------------------------

#[cfg(feature = "openapi")]
mod openapi {
    use super::*;
    use vld::openapi::JsonSchema;

    vld::schema! {
        #[derive(Debug, serde::Serialize)]
        pub struct Address {
            pub city: String => vld::string(),
        }
    }

    vld::schema! {
        #[derive(Debug)]
        pub struct Person {
            pub home: Address => vld::nested!(Address),
        }
    }

    #[test]
    fn nullable_and_unions_use_any_of() {
        let js = vld::string().optional().json_schema();
        assert_eq!(js["anyOf"][1], json!({"type": "null"}));
        assert!(js.get("oneOf").is_none());
        // no double wrapping
        let js = vld::string().nullable().optional().json_schema();
        assert_eq!(js["anyOf"].as_array().unwrap().len(), 2);
        let js = vld::union(vld::string(), vld::number()).json_schema();
        assert_eq!(js["anyOf"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn openapi_document_includes_nested_components() {
        let doc = Person::to_openapi_document();
        let schemas = &doc["components"]["schemas"];
        assert_eq!(
            schemas["Person"]["properties"]["home"]["$ref"],
            "#/components/schemas/Address"
        );
        assert_eq!(schemas["Address"]["properties"]["city"]["type"], "string");
    }

    #[test]
    fn zobject_collects_nested_schemas() {
        use vld::openapi::CollectNestedSchemas;
        let obj = vld::object().field("home", vld::nested!(Address).optional());
        let mut out = Vec::new();
        obj.collect_nested_schemas(&mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, "Address");
    }

    #[test]
    fn empty_required_is_omitted() {
        vld::schema! {
            pub struct AllOptional {
                pub a: Option<String> => vld::string().optional(),
            }
        }
        assert!(AllOptional::json_schema().get("required").is_none());
        assert!(vld::object().to_json_schema().get("required").is_none());
    }

    #[test]
    fn field_optional_schema_allows_null() {
        let js = vld::object()
            .field_optional("n", vld::object().field("x", vld::string()))
            .to_json_schema();
        assert_eq!(js["properties"]["n"]["anyOf"][1], json!({"type": "null"}));
    }

    vld::schema! {
        pub struct Exotic {
            pub kind: String => vld::literal("a"),
            pub pair: (String, f64) => (vld::string(), vld::number()),
            pub m: std::collections::HashMap<String, f64> => vld::map(vld::string(), vld::number()),
            pub du: serde_json::Value => vld::discriminated_union("t")
                .variant_str("x", vld::object().field("t", vld::literal("x"))),
            pub lazy: String => vld::lazy(vld::string),
            pub custom: i64 => vld::custom(|v: &serde_json::Value| v.as_i64().ok_or("int".to_string())),
            pub piped: f64 => vld::string().pipe(vld::any()).transform(|_| 1.0),
        }
    }

    #[test]
    fn schema_macro_accepts_exotic_field_schemas() {
        let js = Exotic::json_schema();
        let pair = &js["properties"]["pair"];
        assert_eq!(pair["prefixItems"][0]["type"], "string");
        assert_eq!(pair["items"], json!(false));
        assert_eq!(pair["minItems"], 2);
        assert_eq!(js["properties"]["m"]["type"], "array");
    }

    vld::schema! {
        #[into_params(parameter_in = Query, rename_all = "camelCase")]
        pub struct QueryWithExtras {
            pub q: String => vld::string(),
        }
    }
    vld::schema! {
        #[into_params(rename_all = "camelCase", parameter_in = Header)]
        pub struct HeaderLast {
            pub q: String => vld::string(),
        }
    }

    #[test]
    fn param_in_tolerates_extra_args() {
        use vld::openapi::OpenApiParameterIn;
        assert_eq!(QueryWithExtras::parameter_in(), Some("query"));
        assert_eq!(HeaderLast::parameter_in(), Some("header"));
    }
}

// ---------------------------------------------------------------------------
// derive
// ---------------------------------------------------------------------------

#[cfg(feature = "derive")]
mod derive {
    use super::*;
    use vld::schema::VldParse;
    use vld::Validate;

    /// No Default / Serialize: must still derive (lenient is opt-in).
    #[derive(Debug)]
    pub struct Opaque(pub String);

    #[derive(Debug, Validate)]
    pub struct HasOpaque {
        #[vld(vld::string().transform(Opaque))]
        pub inner: Opaque,
    }

    #[test]
    fn derive_without_default_or_serialize_fields() {
        let v = HasOpaque::parse_value(&json!({"inner": "x"})).unwrap();
        assert_eq!(v.inner.0, "x");
    }

    #[derive(Debug, Validate)]
    pub struct RawDerive {
        #[vld(vld::string())]
        pub r#type: String,
    }

    #[test]
    fn derive_strips_raw_identifier_prefix() {
        assert_eq!(
            RawDerive::parse_value(&json!({"type": "a"}))
                .unwrap()
                .r#type,
            "a"
        );
    }

    #[derive(Debug, serde::Deserialize, Validate)]
    #[serde(rename_all = "UPPERCASE")]
    pub struct Serdeish {
        #[vld(vld::string())]
        pub first_name: String,
        #[serde(rename(serialize = "ser", deserialize = "de"))]
        #[vld(vld::string())]
        pub renamed: String,
        #[serde(skip)]
        pub skipped: Vec<u8>,
        #[serde(default)]
        #[vld(vld::number().int().min(10))]
        pub defaulted: i64,
        #[serde(default = "seven")]
        #[vld(vld::number().int())]
        pub with_fn: i64,
    }

    fn seven() -> i64 {
        7
    }

    #[test]
    fn derive_serde_attributes() {
        let v = Serdeish::parse_value(&json!({"FIRST_NAME": "a", "de": "b", "skipped": "ignored"}))
            .unwrap();
        assert_eq!(v.first_name, "a");
        assert_eq!(v.renamed, "b");
        assert!(v.skipped.is_empty());
        assert_eq!(v.defaulted, 0);
        assert_eq!(v.with_fn, 7);
        // present keys are still validated
        assert!(
            Serdeish::parse_value(&json!({"FIRST_NAME": "a", "de": "b", "DEFAULTED": 1})).is_err()
        );
    }

    #[derive(Debug, Validate)]
    pub struct Page<T: VldParse> {
        #[vld(vld::array(vld::nested(T::vld_parse_value)))]
        pub items: Vec<T>,
    }

    #[derive(Debug, Validate)]
    pub struct Item {
        #[vld(vld::number().int())]
        pub id: i64,
    }

    #[test]
    fn derive_supports_generics() {
        let p = Page::<Item>::parse_value(&json!({"items": [{"id": 1}]})).unwrap();
        assert_eq!(p.items[0].id, 1);
    }

    #[cfg(feature = "openapi")]
    mod openapi {
        use super::*;

        #[derive(Debug, Validate)]
        pub struct DerivedAddr {
            #[vld(vld::string())]
            pub city: String,
        }

        #[derive(Debug, Validate)]
        pub struct DerivedUser {
            #[vld(vld::nested!(DerivedAddr))]
            pub addr: DerivedAddr,
            #[vld(vld::nested!(Page<Item>).optional())]
            pub page: Option<Page<Item>>,
        }

        #[test]
        fn derive_supports_nested_macro_and_registers_components() {
            let names: Vec<&str> = DerivedUser::__vld_nested_schemas()
                .iter()
                .map(|(n, _)| *n)
                .collect();
            assert_eq!(names, ["DerivedAddr", "Page_Item_"]);
            let doc = DerivedUser::to_openapi_document();
            let schemas = &doc["components"]["schemas"];
            assert!(schemas["DerivedAddr"].is_object());
            assert!(schemas["Page_Item_"].is_object());
            assert_eq!(
                schemas["DerivedUser"]["properties"]["page"]["anyOf"][0]["$ref"],
                "#/components/schemas/Page_Item_"
            );
        }

        #[test]
        fn derive_default_fields_not_required() {
            let js = Serdeish::json_schema();
            let req: Vec<&str> = js["required"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect();
            assert_eq!(req, ["FIRST_NAME", "de"]);
            assert!(js["properties"].get("skipped").is_none());
        }
    }

    #[test]
    fn derive_generates_validate_and_is_valid() {
        #[derive(serde::Serialize)]
        struct Raw {
            id: i64,
        }
        assert!(Item::is_valid(&Raw { id: 1 }));
        assert!(Item::validate(&json!({"id": "x"})).is_err());
    }
}

// ---------------------------------------------------------------------------
// diff
// ---------------------------------------------------------------------------

#[cfg(feature = "diff")]
mod diff {
    use serde_json::json;
    use vld::diff::{diff_schemas, ChangeKind};

    fn obj(prop: serde_json::Value, required: bool) -> serde_json::Value {
        let mut v = json!({"type": "object", "properties": {"a": prop}});
        if required {
            v["required"] = json!(["a"]);
        }
        v
    }

    #[test]
    fn making_field_optional_is_not_a_type_change() {
        let old = obj(json!({"type": "string"}), true);
        let new = obj(
            json!({"anyOf": [{"type": "string"}, {"type": "null"}]}),
            false,
        );
        let d = diff_schemas(&old, &new);
        assert!(!d.has_breaking(), "{d}");
        assert!(d
            .changes
            .iter()
            .any(|c| c.description == "Now accepts null"));
    }

    #[test]
    fn removing_null_is_breaking() {
        let d = diff_schemas(
            &json!({"anyOf": [{"type": "string"}, {"type": "null"}]}),
            &json!({"type": "string"}),
        );
        assert!(d.has_breaking());
    }

    #[test]
    fn changes_inside_optional_field_are_visible() {
        let old = obj(
            json!({"anyOf": [{"type": "string", "maxLength": 10}, {"type": "null"}]}),
            false,
        );
        let new = obj(
            json!({"anyOf": [{"type": "string", "maxLength": 5}, {"type": "null"}]}),
            false,
        );
        let d = diff_schemas(&old, &new);
        assert_eq!(d.changes.len(), 1, "{d}");
        assert_eq!(d.changes[0].path, "properties.a.maxLength");
        assert_eq!(d.changes[0].kind, ChangeKind::Breaking);
    }

    #[test]
    fn union_branches_and_refs_are_diffed() {
        let d = diff_schemas(
            &json!({"anyOf": [{"type": "string"}, {"type": "number"}]}),
            &json!({"anyOf": [{"type": "string", "minLength": 1}, {"type": "number"}, {"type": "boolean"}]}),
        );
        assert!(d.changes.iter().any(|c| c.path == "anyOf[0].minLength"));
        assert!(d
            .changes
            .iter()
            .any(|c| c.path == "anyOf" && c.kind == ChangeKind::NonBreaking));

        let d = diff_schemas(
            &json!({"$ref": "#/components/schemas/A"}),
            &json!({"$ref": "#/components/schemas/B"}),
        );
        assert!(d.has_breaking());
    }

    #[test]
    fn new_required_property_reported_once() {
        let old = json!({"type": "object", "properties": {}});
        let new = obj(json!({"type": "string"}), true);
        let d = diff_schemas(&old, &new);
        assert_eq!(d.changes.len(), 1, "{d}");
        assert_eq!(d.changes[0].kind, ChangeKind::Breaking);
    }

    #[test]
    fn additional_properties_schema_is_diffed() {
        let d = diff_schemas(
            &json!({"type": "object", "additionalProperties": {"type": "string"}}),
            &json!({"type": "object", "additionalProperties": {"type": "number"}}),
        );
        assert!(d.has_breaking());
        let d = diff_schemas(
            &json!({"type": "object"}),
            &json!({"type": "object", "additionalProperties": {"type": "number"}}),
        );
        assert!(d.has_breaking());
        let d = diff_schemas(
            &json!({"type": "object", "additionalProperties": {"type": "number"}}),
            &json!({"type": "object"}),
        );
        assert!(!d.has_breaking());
    }
}
