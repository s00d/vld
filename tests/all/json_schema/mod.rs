//! JSON Schema / OpenAPI output. Whole documents are insta snapshots
//! (`snapshots/`, maps sorted); `differential` checks that vld and the
//! `jsonschema` crate agree on what the emitted schemas accept.

mod differential;

use serde_json::{json, Map, Value};
use vld::openapi::JsonSchema;
use vld::prelude::*;

/// Snapshot with sorted map keys, so serde_json's `preserve_order` (enabled
/// by some workspace crates) cannot change the output.
macro_rules! snap {
    ($name:expr, $value:expr) => {
        insta::with_settings!({ sort_maps => true, prepend_module_to_snapshot => false }, {
            let value = $value;
            insta::assert_json_snapshot!($name, value);
        })
    };
}

/// `{name: {"schema": ..., "required": bool}}` for a list of schemas.
macro_rules! schema_table {
    ($($name:literal => $schema:expr),* $(,)?) => {{
        let mut m = Map::new();
        $(
            let s = $schema;
            m.insert(
                $name.to_string(),
                json!({"schema": s.json_schema(), "required": s.is_required()}),
            );
        )*
        Value::Object(m)
    }};
}

fn required_keys(js: &Value) -> Vec<&str> {
    js["required"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default()
}

#[test]
fn primitives() {
    snap!(
        "primitives",
        schema_table! {
            "string" => vld::string().min(3).max(50),
            "string_strictest_bounds" => vld::string().min(2).min(4).max(10).max(8),
            "string_email" => vld::string().email(),
            "string_email_lowercase" => vld::string().email().lowercase(),
            "string_uuid" => vld::string().uuid(),
            "string_prefix_suffix" => vld::string().starts_with("a.b").ends_with("z"),
            "number" => vld::number().min(0.0).max(100.0),
            "number_strictest_min" => vld::number().min(5.0).min(0.0),
            "number_positive" => vld::number().positive(),
            "int" => vld::number().int().min(0).max(100),
            "int_non_positive_safe" => vld::number().int().non_positive().safe(),
            "boolean" => vld::boolean(),
            "enum" => vld::enumeration(&["admin", "user"]),
            "literal" => vld::literal("admin"),
            "any" => vld::any(),
            "bytes_base64_len4" => vld::bytes().base64().len(4),
            "bytes_hex_min3" => vld::bytes().hex().min_len(3),
            "duration_min5" => vld::duration().min_secs(5),
            "path" => vld::path(),
            "socket_addr" => vld::socket_addr(),
            "json_value_object" => vld::json_value().object(),
        }
    );
}

#[test]
fn collections() {
    snap!(
        "collections",
        schema_table! {
            "array" => vld::array(vld::string().min(1)).min_len(1).max_len(10),
            "array_contains" => vld::array(vld::number()).contains(5),
            "array_min_contains_without_contains" => vld::array(vld::number()).min_contains(2),
            "record" => vld::record(vld::number().positive()).min_keys(1).max_keys(3),
            // Duplicates are merged, not rejected: no uniqueItems / maxItems.
            "set" => vld::set(vld::string()).min_size(1).max_size(5),
            "map" => vld::map(vld::string(), vld::number()),
            "tuple" => (vld::string(), vld::number()),
        }
    );
}

/// Requiredness and nullability through every modifier and wrapper stack.
#[test]
fn modifiers_and_wrapper_stacks() {
    snap!(
        "modifiers",
        schema_table! {
            "plain" => vld::string().min(1),
            "optional" => vld::string().min(2).email().optional(),
            "nullish" => vld::number().int().min(0).nullish(),
            "nullable" => vld::number().min(0.0).max(100.0).nullable(),
            "nullable_optional_no_double_wrap" => vld::string().nullable().optional(),
            "with_default" => vld::string().min(1).with_default("hello".into()),
            "with_default_bool" => vld::boolean().with_default(false),
            "catch" => vld::string().min(1).catch("fallback".into()),
            "describe" => vld::string().describe("User display name"),
            "refine_passes_inner" => vld::string().refine(|s| s.starts_with('A'), "A"),
            "transform_passes_inner" => vld::string().transform(|s| s.len()),
            "optional_stack" => vld::string()
                .min(3)
                .optional()
                .describe("nickname")
                .message("bad nickname")
                .refine(|v| v.as_ref().is_none_or(|s| s.len() >= 3), "short")
                .catch(None)
                .transform(|v| v.map(|s| s.to_uppercase())),
            "nullish_stack" => vld::number().int().nullish().message("m").describe("optional age"),
            "default_stack" => vld::string()
                .with_default("user".into())
                .describe("role")
                .refine(|s| !s.is_empty(), "empty"),
            "nullable_stack" => vld::string()
                .email()
                .nullable()
                .describe("contact")
                .message("m")
                .super_refine(|_v, _e| {}),
            "plain_stack" => vld::string()
                .min(2)
                .describe("name")
                .refine(|s| s.chars().all(char::is_alphabetic), "letters")
                .message("m")
                .catch("Anon".into()),
            "preprocess_optional" => vld::preprocess(|v: &Value| v.clone(), vld::string().optional()),
            "preprocess_required" => vld::preprocess(|v: &Value| v.clone(), vld::string().min(1)),
        }
    );
}

#[test]
fn combinators() {
    let du = vld::discriminated_union("type")
        .variant_str("a", vld::object().field("type", vld::literal("a")))
        .variant_str("b", vld::object().field("type", vld::literal("b")));
    snap!(
        "combinators",
        schema_table! {
            "union" => vld::union(vld::string(), vld::number()),
            "union3" => vld::union3(vld::string(), vld::number(), vld::boolean()),
            "intersection" => vld::intersection(vld::string().min(3), vld::string().max(10)),
            "discriminated_union" => du,
            "lazy" => vld::lazy(vld::string),
        }
    );
}

#[test]
fn dynamic_object_pipeline() {
    let base = vld::object()
        .field("id", vld::number().int().positive())
        .field(
            "title",
            vld::string()
                .min(1)
                .max(80)
                .describe("post title")
                .message("bad title"),
        )
        .field(
            "slug",
            vld::string()
                .min(3)
                .optional()
                .describe("url slug")
                .refine(|v| v.as_ref().is_none_or(|s| !s.contains(' ')), "spaces"),
        )
        .field_optional("draft_note", vld::string().max(200))
        .field(
            "status",
            vld::string()
                .with_default("draft".into())
                .describe("workflow"),
        )
        .field("summary", vld::string().max(500).nullish())
        .field(
            "body",
            vld::string().min(1).nullable().describe("markdown body"),
        )
        .field(
            "score",
            vld::number()
                .min(0.0)
                .max(10.0)
                .optional()
                .catch(None)
                .describe("editor score"),
        )
        .strict();
    let three = || {
        vld::object()
            .field("id", vld::number().int())
            .field("title", vld::string().optional())
            .field("body", vld::string().nullable())
    };
    let left = vld::object()
        .field("name", vld::string())
        .field("age", vld::number().int());
    let right = vld::object()
        .field(
            "age",
            vld::number().int().optional().describe("optional age"),
        )
        .field("city", vld::string().min(1));
    snap!(
        "object_pipeline",
        json!({
            "strict_mixed": base.json_schema(),
            "partial": three().partial().json_schema(),
            "partial_required": three().partial().required().json_schema(),
            "pick": three().pick(&["title", "body"]).json_schema(),
            "omit": three().omit("id").json_schema(),
            "extend_later_wins": left.extend(right).json_schema(),
            "catchall": vld::object()
                .field("email", vld::string().email())
                .field("age", vld::number().int().optional())
                .catchall(vld::number().min(0.0))
                .json_schema(),
            "field_optional_object": vld::object()
                .field_optional("n", vld::object().field("x", vld::string()))
                .json_schema(),
            "empty": vld::object().json_schema(),
        })
    );
    // Spot checks that matter more than the shape.
    assert_eq!(required_keys(&base.json_schema()), ["id", "title"]);
    assert!(required_keys(&three().partial().json_schema()).is_empty());
    assert!(vld::object().to_json_schema().get("required").is_none());
}

vld::schema! {
    #[derive(Debug)]
    struct ProfileAddress {
        street: String => vld::string().min(1).describe("street line"),
        city: String => vld::string().min(2),
        zip: Option<String> => vld::string().min(4).optional().describe("postal code"),
    }
}

vld::schema! {
    #[derive(Debug)]
    struct CreateProfile {
        name: String => vld::string().min(2).max(100).describe("display name").message("invalid name")
            .refine(|s| !s.trim().is_empty(), "blank"),
        email: String => vld::string().email(),
        tags: Vec<String> => vld::array(vld::string().min(1)).min_len(1),
        address: ProfileAddress => vld::nested!(ProfileAddress),
        age: Option<i64> => vld::number().int().gte(0).lte(150).optional().describe("years"),
        nickname: Option<String> => vld::string().optional().describe("display nickname")
            .message("bad nickname").catch(None),
        role: String => vld::string().with_default("user".into()).describe("account role"),
        bio: Option<String> => vld::string().max(500).nullish(),
        locale: Option<String> => vld::string().min(2).optional()
            .refine(|v| v.as_ref().is_none_or(|s| s.len() == 2), "xx"),
        meta: Option<String> => vld::string().nullable().describe("opaque meta"),
        notes: Option<String> => vld::string().max(200).nullable().message("bad notes")
            .super_refine(|_v, _e| {}),
        is_current: bool as "isCurrent" => vld::boolean().with_default(false),
    }
}

#[test]
fn schema_macro_document() {
    snap!(
        "schema_macro_create_profile",
        CreateProfile::to_openapi_document()
    );
    assert_eq!(
        required_keys(&CreateProfile::json_schema()),
        ["name", "email", "tags", "address"]
    );
    let nested: Vec<&str> = CreateProfile::__vld_nested_schemas()
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    assert_eq!(nested, ["ProfileAddress"]);

    // Runtime behaviour matches the documented contract: a missing nullable
    // field parses as None, so `meta` is not required.
    let base = json!({
        "name": "Ada", "email": "ada@example.com", "tags": ["rust"],
        "address": {"street": "1 Main", "city": "NY"}
    });
    let p = CreateProfile::parse_value(&base).unwrap();
    assert_eq!(
        (p.meta, p.notes, p.role.as_str(), p.is_current),
        (None, None, "user", false)
    );
}

#[cfg(feature = "derive")]
#[test]
fn derive_document() {
    #[derive(Debug, vld::Validate)]
    #[allow(dead_code)]
    struct PatchUser {
        #[vld(vld::string().min(2).max(50).describe("full name"))]
        name: String,
        #[vld(vld::string().email())]
        email: String,
        #[vld(vld::number().int().gte(0).lte(150).optional().describe("years"))]
        age: Option<i64>,
        #[vld(vld::string().optional().describe("nick").message("bad nick").catch(None))]
        nickname: Option<String>,
        #[vld(vld::string().with_default("user".into()).describe("role"))]
        role: String,
        #[vld(vld::string().max(400).nullish())]
        bio: Option<String>,
        #[vld(vld::string().nullable().describe("opaque"))]
        meta: Option<String>,
        #[vld(vld::array(vld::string().min(1)).optional().describe("labels"))]
        labels: Option<Vec<String>>,
    }
    snap!("derive_patch_user", PatchUser::to_openapi_document());
    assert_eq!(required_keys(&PatchUser::json_schema()), ["name", "email"]);
}

#[test]
fn openapi_helpers() {
    use vld::openapi::{to_openapi_document, to_openapi_document_multi};
    snap!(
        "openapi_helpers",
        json!({
            "single": to_openapi_document("Email", &vld::string().email().json_schema()),
            "multi": to_openapi_document_multi(&[
                ("Name", vld::string().min(1).json_schema()),
                ("Age", vld::number().int().min(0).json_schema()),
            ]),
        })
    );
}

// --- nested component collection -------------------------------------------------

vld::schema! {
    #[derive(Debug, serde::Serialize)]
    pub struct Leaf {
        pub id: String => vld::string().min(1),
    }
}
vld::schema! {
    #[derive(Debug)]
    pub struct Left {
        pub leaf: Leaf => vld::nested!(Leaf),
    }
}
vld::schema! {
    #[derive(Debug)]
    pub struct Right {
        pub leaves: Option<Vec<Leaf>> => vld::array(vld::nested!(Leaf)).optional(),
    }
}
vld::schema! {
    #[derive(Debug)]
    pub struct Root {
        pub left: Left => vld::nested!(Left).message("bad left"),
        pub right: Right => vld::nested!(Right),
    }
}

/// Transitive (grandchildren), deduplicated (diamond), and found behind
/// wrappers (`message`, `optional`, arrays).
#[test]
fn nested_schemas_transitive_dedup_and_wrapped() {
    let mut names: Vec<&str> = Root::__vld_nested_schemas()
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    names.sort();
    assert_eq!(names, ["Leaf", "Left", "Right"]);

    let doc = Root::to_openapi_document();
    let schemas = &doc["components"]["schemas"];
    let mut keys: Vec<&String> = schemas.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, ["Leaf", "Left", "Right", "Root"]);
    assert_eq!(
        schemas["Root"]["properties"]["left"]["$ref"],
        "#/components/schemas/Left"
    );

    use vld::openapi::CollectNestedSchemas;
    let obj = vld::object().field("home", vld::nested!(Leaf).optional());
    let mut out = Vec::new();
    obj.collect_nested_schemas(&mut out);
    assert_eq!(out.iter().map(|(n, _)| *n).collect::<Vec<_>>(), ["Leaf"]);
}

#[test]
fn into_params_parameter_in() {
    use vld::openapi::OpenApiParameterIn;
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
    assert_eq!(QueryWithExtras::parameter_in(), Some("query"));
    assert_eq!(HeaderLast::parameter_in(), Some("header"));
}

#[test]
fn exotic_field_schemas_in_schema_macro() {
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
    snap!("schema_macro_exotic", Exotic::json_schema());
}

#[cfg(feature = "net")]
#[test]
fn with_default_ip_network_emits_default() {
    let n = vld::ip_network().parse(r#""10.0.0.0/8""#).unwrap();
    assert_eq!(
        vld::ip_network().with_default(n).json_schema()["default"],
        "10.0.0.0/8"
    );
}

#[cfg(feature = "decimal")]
#[test]
fn decimal_schema() {
    let n = vld::decimal().parse(r#""1.5""#).unwrap();
    assert_eq!(
        vld::decimal().with_default(n).json_schema()["default"],
        "1.5"
    );
    assert_eq!(
        vld::decimal().positive().json_schema()["x-exclusiveMinDecimal"],
        "0"
    );
}

#[test]
fn nullable_schema_helper() {
    use vld::openapi::nullable_schema;
    let n = nullable_schema(json!({"type": "string"}));
    assert_eq!(n, json!({"anyOf": [{"type": "string"}, {"type": "null"}]}));
    // Already nullable: not wrapped twice.
    assert_eq!(nullable_schema(n.clone()), n);
}

#[test]
fn nested_named_registers_a_component() {
    fn leaf_schema() -> Value {
        Leaf::json_schema()
    }
    vld::schema! {
        #[derive(Debug)]
        pub struct Holder {
            pub leaf: Leaf => vld::nested_named("Custom.Leaf", Leaf::parse_value, Some(leaf_schema), None),
        }
    }
    let doc = Holder::to_openapi_document();
    assert_eq!(
        doc["components"]["schemas"]["Holder"]["properties"]["leaf"]["$ref"],
        "#/components/schemas/Custom.Leaf"
    );
    assert_eq!(
        doc["components"]["schemas"]["Custom.Leaf"],
        Leaf::json_schema()
    );
    assert_eq!(
        Holder::parse(r#"{"leaf": {"id": "x"}}"#).unwrap().leaf.id,
        "x"
    );
}

#[test]
#[allow(deprecated)]
fn field_schema_is_field() {
    let a = vld::object()
        .field_schema("a", vld::string().email())
        .json_schema();
    let b = vld::object()
        .field("a", vld::string().email())
        .json_schema();
    assert_eq!(a, b);
}
