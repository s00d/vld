use vld::schema::VldSchema;
use vld_ts::{ToOpenApi, ToRefs, ToValibot, ToZod};

#[test]
fn vld_schema_string_with_constraints() {
    let zod = vld_ts::to_zod(&vld::string().min(2).max(50).email());
    assert_eq!(zod, "z.string().min(2).max(50).email()");
}

#[test]
fn vld_schema_integer_constraints() {
    let zod = vld_ts::to_zod(&vld::number().int().min(0).max(100));
    assert_eq!(zod, "z.number().int().min(0).max(100)");
}

#[test]
fn vld_schema_nullable() {
    let zod = vld_ts::to_zod(&vld::string().nullable());
    assert_eq!(zod, "z.string().nullable()");
}

#[test]
fn vld_schema_union() {
    let zod = vld_ts::to_zod(&vld::union(vld::string(), vld::number()));
    assert_eq!(zod, "z.union([z.string(), z.number()])");
}

vld::schema! {
    #[derive(Debug)]
    pub struct TsAddress {
        pub city: String => vld::string().min(1),
        pub zip: String => vld::string().min(5).max(10),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct TsOrder {
        pub id: String => vld::string().min(1),
        pub shipping: TsAddress => vld::nested!(TsAddress),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct TsRenamed {
        pub first_name: String as "firstName" => vld::string().min(1),
        pub is_active: bool as "isActive" => vld::boolean(),
    }
}

vld_ts::impl_to_zod!(TsOrder);
vld_ts::impl_to_zod!(TsRenamed);
vld_ts::impl_to_openapi!(TsOrder);
vld_ts::impl_to_openapi!(TsRenamed);
vld_ts::impl_to_valibot!(TsOrder);
vld_ts::impl_to_valibot!(TsRenamed);

vld::schema! {
    #[derive(Debug)]
    pub struct TsCustomNamed {
        pub name: String => vld::string().min(1),
    }
}

vld_ts::impl_to_zod!(TsCustomNamed, "RequestBody");
vld_ts::impl_to_openapi!(TsCustomNamed, "RequestBody");
vld_ts::impl_to_valibot!(TsCustomNamed, "RequestBody");

/// Full generated code for typed structs (nested refs, renames, custom
/// export names) — one text snapshot per generator.
#[test]
fn generated_code_snapshots() {
    let zod = [
        TsOrder::to_zod(),
        TsRenamed::to_zod(),
        TsCustomNamed::to_zod(),
        TsOrder::to_zod_export(),
        TsCustomNamed::to_zod_export(),
        vld_ts::to_zod(
            &vld::array(vld::string().min(1))
                .max_len(10)
                .describe("Tags"),
        ),
    ]
    .join("\n\n");
    insta::assert_snapshot!("zod", zod);

    let valibot = [
        TsOrder::to_valibot(),
        TsRenamed::to_valibot(),
        TsCustomNamed::to_valibot(),
        TsCustomNamed::to_valibot_export(),
        vld_ts::to_valibot(&vld::string().min(2).email()),
    ]
    .join("\n\n");
    insta::assert_snapshot!("valibot", valibot);

    insta::with_settings!({ sort_maps => true }, {
        insta::assert_json_snapshot!(
            "openapi",
            serde_json::json!({
                "order": TsOrder::to_openapi(),
                "renamed": TsRenamed::to_openapi(),
                "custom_named": TsCustomNamed::to_openapi(),
            })
        );
    });
}

#[test]
fn to_openapi_from_schema_instance() {
    let schema = vld_ts::to_openapi(&vld::string().email());
    assert_eq!(schema["type"], "string");
    assert_eq!(schema["format"], "email");
}

#[test]
fn type_to_openapi_single_with_nested() {
    let doc = TsOrder::to_openapi();
    assert_eq!(doc["type"], "object");
    assert_eq!(
        doc["properties"]["shipping"]["$ref"],
        "#/components/schemas/TsAddress"
    );
    let refs = vld_ts::openapi_refs(&doc);
    assert!(refs.contains(&"#/components/schemas/TsAddress".to_string()));
    let refs2 = TsOrder::to_refs();
    assert!(refs2.contains(&"#/components/schemas/TsAddress".to_string()));
}

// ---------------------------------------------------------------------------
// Regression tests: valid JS output, preserved metadata, Zod/Valibot parity
// ---------------------------------------------------------------------------

use serde_json::json;
use vld_ts::{__to_valibot_from_value as valibot_of, __to_zod_from_value as zod_of};

#[test]
fn enum_literals_are_escaped() {
    let schema = vld::enumeration(&["a\"b", "c\\d"]);
    let zod = vld_ts::to_zod(&schema);
    assert_eq!(zod, r#"z.union([z.literal("a\"b"), z.literal("c\\d")])"#);
    let valibot = vld_ts::to_valibot(&schema);
    assert_eq!(
        valibot,
        r#"v.union([v.literal("a\"b"), v.literal("c\\d")])"#
    );
}

#[test]
fn pattern_uses_regexp_constructor() {
    let schema = json!({"type": "string", "pattern": "^a/b\"c$"});
    assert_eq!(
        zod_of(&schema),
        r#"z.string().regex(new RegExp("^a/b\"c$"))"#
    );
    assert_eq!(
        valibot_of(&schema),
        r#"v.pipe(v.string(), v.regex(new RegExp("^a/b\"c$")))"#
    );
}

#[test]
fn description_is_json_escaped() {
    let zod = vld_ts::to_zod(&vld::string().describe("line1\nback\\slash \"q\""));
    assert_eq!(zod, r#"z.string().describe("line1\nback\\slash \"q\"")"#);
}

#[test]
fn non_identifier_keys_are_quoted() {
    let schema = json!({
        "type": "object",
        "required": ["first-name", "ok_1"],
        "properties": {
            "first-name": {"type": "string"},
            "ok_1": {"type": "string"}
        }
    });
    let zod = zod_of(&schema);
    assert!(zod.contains("  \"first-name\": z.string()"), "{zod}");
    assert!(zod.contains("  ok_1: z.string()"), "{zod}");
    let valibot = valibot_of(&schema);
    assert!(
        valibot.contains("  \"first-name\": v.string()"),
        "{valibot}"
    );
}

#[test]
fn default_is_emitted() {
    let schema = vld::number().int().with_default(5);
    assert_eq!(vld_ts::to_zod(&schema), "z.number().int().default(5)");
    assert_eq!(
        vld_ts::to_valibot(&schema),
        "v.optional(v.pipe(v.number(), v.integer()), 5)"
    );

    // In an object, a defaulted field is not additionally wrapped in optional.
    let obj = json!({
        "type": "object",
        "required": [],
        "properties": {"role": {"type": "string", "default": "user"}}
    });
    assert!(zod_of(&obj).contains("role: z.string().default(\"user\")\n"));
    assert!(valibot_of(&obj).contains("role: v.optional(v.string(), \"user\")\n"));
}

#[test]
fn description_kept_on_combinators_refs_enums_and_booleans() {
    let nullable = vld_ts::to_zod(&vld::string().nullable().describe("N"));
    assert_eq!(nullable, "z.string().nullable().describe(\"N\")");

    let reff = json!({"$ref": "#/components/schemas/A", "description": "R"});
    assert_eq!(zod_of(&reff), "z.lazy(() => ASchema).describe(\"R\")");
    assert_eq!(
        valibot_of(&reff),
        "v.pipe(v.lazy(() => ASchema), v.description(\"R\"))"
    );

    let en = vld::enumeration(&["a"]).describe("E");
    assert_eq!(vld_ts::to_zod(&en), "z.literal(\"a\").describe(\"E\")");

    let all = json!({"allOf": [{"type": "string"}, {"type": "number"}], "description": "I"});
    assert_eq!(
        zod_of(&all),
        "z.intersection(z.string(), z.number()).describe(\"I\")"
    );

    assert_eq!(
        vld_ts::to_zod(&vld::boolean().describe("B")),
        "z.boolean().describe(\"B\")"
    );
    assert_eq!(
        zod_of(&json!({"type": "null", "description": "Z"})),
        "z.null().describe(\"Z\")"
    );
}

#[test]
fn any_of_handled_like_one_of() {
    let nullable = json!({"anyOf": [{"type": "string"}, {"type": "null"}]});
    assert_eq!(zod_of(&nullable), "z.string().nullable()");
    assert_eq!(valibot_of(&nullable), "v.nullable(v.string())");

    let union = json!({"anyOf": [{"type": "string"}, {"type": "number"}]});
    assert_eq!(zod_of(&union), "z.union([z.string(), z.number()])");
}

#[test]
fn type_arrays_are_supported() {
    let schema = json!({"type": ["string", "null"], "minLength": 1});
    assert_eq!(zod_of(&schema), "z.string().min(1).nullable()");
    assert_eq!(
        valibot_of(&schema),
        "v.nullable(v.pipe(v.string(), v.minLength(1)))"
    );
    let multi = json!({"type": ["string", "integer"]});
    assert_eq!(zod_of(&multi), "z.union([z.string(), z.number().int()])");
}

#[test]
fn catchall_and_unknown_key_handling() {
    let schema = json!({
        "type": "object",
        "required": ["a"],
        "properties": {"a": {"type": "string"}},
        "additionalProperties": {"type": "number"}
    });
    let zod = zod_of(&schema);
    assert!(zod.ends_with(".catchall(z.number())"), "{zod}");
    let valibot = valibot_of(&schema);
    assert!(valibot.starts_with("v.objectWithRest("), "{valibot}");
    assert!(valibot.ends_with(", v.number())"), "{valibot}");

    // additionalProperties: true → both strip unknown keys.
    let open = json!({
        "type": "object",
        "required": ["a"],
        "properties": {"a": {"type": "string"}},
        "additionalProperties": true
    });
    assert_eq!(zod_of(&open), "z.object({\n  a: z.string()\n})");
    assert_eq!(valibot_of(&open), "v.object({\n  a: v.string()\n})");

    let strict = vld::object().field("a", vld::string()).strict();
    assert!(vld_ts::to_zod(&strict).ends_with(".strict()"));
    assert!(vld_ts::to_valibot(&strict).starts_with("v.strictObject("));
}

#[test]
fn valibot_array_length_uses_min_length() {
    let ts = vld_ts::to_valibot(&vld::array(vld::string()).min_len(1).max_len(3));
    assert_eq!(
        ts,
        "v.pipe(v.array(v.string()), v.minLength(1), v.maxLength(3))"
    );
}
