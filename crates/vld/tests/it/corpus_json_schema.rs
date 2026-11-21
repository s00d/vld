//! Corpus: the JSON Schema / OpenAPI vld emits. Whole documents are insta
//! snapshots (`snapshots/`, maps sorted); exact schemas of single types are
//! tables; a differential checks that vld and the `jsonschema` crate (draft
//! 2020-12, formats asserted) agree on what the emitted schemas accept, with
//! the accepted divergences listed in [`KNOWN_DIVERGENCES`].

use serde_json::{Map, Value, json};
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

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)]
enum Role {
    Admin,
    User,
}

// ---------------------------------------------------------------------------
// Snapshot documents
// ---------------------------------------------------------------------------

/// Given every primitive, collection, combinator and modifier stack (string
/// bounds and formats, strictest of repeated bounds, numbers, sized ints,
/// booleans, enums, literals, bytes, durations, paths, socket addresses,
/// json values; arrays, records, sets, maps, tuples; unions, intersections,
/// discriminated unions, lazy; optional / nullable / nullish / default /
/// catch / describe / refine / transform / message / super_refine /
/// preprocess and stacks of them)
/// When their JSON Schema and requiredness are emitted
/// Then each table equals its snapshot.
#[test]
fn schema_tables_match_snapshots() {
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

/// Given a strict dynamic object mixing every field modifier, and objects
/// derived by `partial`, `partial().required()`, `pick`, `omit`, `extend`,
/// `catchall`, `field_optional`
/// When their schemas are emitted
/// Then they equal the snapshot; `nullable()` fields are required (the key
/// must be present), a partial object and an empty object require nothing.
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
    assert_eq!(required_keys(&base.json_schema()), ["id", "title", "body"]);
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

/// Given a `CreateProfile` declared with `schema!` using every modifier, a
/// nested component and a renamed field
/// When its OpenAPI document is emitted and bodies are parsed
/// Then the document equals the snapshot, `required` lists exactly the
/// fields that need a key (`nullable` ones included), the nested type is
/// collected, and parsing agrees: absent `nullable` fields are
/// `missing_field`, `null` is accepted, defaults apply.
#[test]
fn schema_macro_document() {
    snap!(
        "schema_macro_create_profile",
        CreateProfile::to_openapi_document()
    );
    assert_eq!(
        required_keys(&CreateProfile::json_schema()),
        ["name", "email", "tags", "address", "meta", "notes"]
    );
    let nested: Vec<&str> = CreateProfile::__vld_nested_schemas()
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    assert_eq!(nested, ["ProfileAddress"]);

    let mut base = json!({
        "name": "Ada", "email": "ada@example.com", "tags": ["rust"],
        "address": {"street": "1 Main", "city": "NY"}
    });
    let e = CreateProfile::parse_value(&base).unwrap_err();
    assert_eq!(
        crate::common::issues(&e),
        [
            (".meta".to_string(), "missing_field".to_string()),
            (".notes".to_string(), "missing_field".to_string())
        ]
    );
    base["meta"] = json!(null);
    base["notes"] = json!(null);
    let p = CreateProfile::parse_value(&base).unwrap();
    assert_eq!(
        (p.meta, p.notes, p.role.as_str(), p.is_current),
        (None, None, "user", false)
    );
}

/// Given a derived `PatchUser` using every modifier
/// When its OpenAPI document is emitted
/// Then it equals the snapshot and requires `name`, `email`, `meta`.
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
    assert_eq!(
        required_keys(&PatchUser::json_schema()),
        ["name", "email", "meta"]
    );
}

/// Given the free OpenAPI helpers and a `schema!` type with exotic field
/// schemas (literal, tuple, map, discriminated union, lazy, custom, pipe)
/// When documents / schemas are emitted
/// Then they equal their snapshots.
#[test]
fn helper_documents_and_exotic_fields() {
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

// ---------------------------------------------------------------------------
// Exact schemas of single types
// ---------------------------------------------------------------------------

/// Given sized integers, `f32`, `number().int()`, adapters, `null`, `never`,
/// `char`, string formats, a 12-tuple, a bounded duration, records with and
/// without key schemas, and the `nullable_schema` helper
/// When their JSON Schema is emitted
/// Then each equals the expected schema: type ranges as `minimum` /
/// `maximum` with a `format`, an exclusive bound at the type edge replacing
/// the inclusive one, the type range winning over `safe()`; `number().int()`
/// without range; adapters `{}` / `{"type":"string"}` unless given a schema;
/// record key schemas as `propertyNames`; `nullable_schema` not wrapping
/// twice.
#[test]
fn exact_schemas_of_types() {
    assert_eq!(
        vld::u8().json_schema(),
        json!({"type": "integer", "format": "uint8", "minimum": 0, "maximum": 255})
    );
    assert_eq!(
        vld::i16().min(-10).json_schema(),
        json!({"type": "integer", "format": "int16", "minimum": -10, "maximum": 32767})
    );
    assert_eq!(
        vld::u64().json_schema(),
        json!({"type": "integer", "format": "uint64", "minimum": 0, "maximum": u64::MAX})
    );
    assert_eq!(vld::i64().json_schema()["minimum"], json!(i64::MIN));
    for (s, f) in [
        (vld::i32().json_schema(), "int32"),
        (vld::u16().json_schema(), "uint16"),
        (vld::u32().json_schema(), "uint32"),
        (vld::i8().json_schema(), "int8"),
        (vld::usize().json_schema(), "uint64"),
        (vld::isize().json_schema(), "int64"),
    ] {
        assert_eq!(s["format"], f);
    }
    let js = vld::u8().positive().json_schema();
    assert_eq!(js["exclusiveMinimum"], 0);
    assert!(js.get("minimum").is_none());
    let js = vld::i32().safe().lt(100).multiple_of(5).json_schema();
    assert_eq!(js["minimum"], i32::MIN);
    assert_eq!(js["exclusiveMaximum"], 100);
    assert!(js.get("maximum").is_none());
    assert_eq!(js["multipleOf"], 5);
    assert_eq!(
        vld::number().int().json_schema(),
        json!({"type": "integer"})
    );
    assert_eq!(
        vld::number().int().min(1).json_schema(),
        json!({"type": "integer", "minimum": 1})
    );
    assert_eq!(
        vld::f32().min(0.5).json_schema(),
        json!({"type": "number", "format": "float", "minimum": 0.5,
               "maximum": f64::from(f32::MAX)})
    );
    assert!(
        vld::number()
            .multiple_of(0.0)
            .json_schema()
            .get("multipleOf")
            .is_none()
    );
    assert_eq!(
        vld::number().multiple_of(2.0).json_schema()["multipleOf"],
        2.0
    );

    assert_eq!(vld::from_serde::<Role>().json_schema(), json!({}));
    let custom = json!({"type": "string", "enum": ["admin", "user"]});
    assert_eq!(
        vld::from_serde::<Role>()
            .schema(custom.clone())
            .json_schema(),
        custom
    );
    assert_eq!(
        vld::from_str::<std::net::IpAddr>().json_schema(),
        json!({"type": "string"})
    );
    assert_eq!(
        vld::from_str::<std::net::IpAddr>()
            .schema(json!({"type": "string", "format": "ip"}))
            .json_schema()["format"],
        "ip"
    );
    assert_eq!(
        vld::enum_of(&[("a", 1), ("b", 2)]).json_schema(),
        json!({"type": "string", "enum": ["a", "b"]})
    );

    assert_eq!(vld::null().json_schema(), json!({"type": "null"}));
    assert_eq!(vld::never().json_schema(), json!({"not": {}}));
    assert_eq!(
        vld::char().json_schema(),
        json!({"type": "string", "minLength": 1, "maxLength": 1})
    );
    assert_eq!(
        vld::string().base64url().json_schema()["format"],
        "base64url"
    );
    assert_eq!(vld::string().json().json_schema()["format"], "json");
    assert_eq!(vld::string().hostname().json_schema()["format"], "hostname");
    let b = vld::boolean;
    let js = (
        b(),
        b(),
        b(),
        b(),
        b(),
        b(),
        b(),
        b(),
        b(),
        b(),
        b(),
        vld::string(),
    )
        .json_schema();
    assert_eq!(js["maxItems"], 12);
    assert_eq!(js["prefixItems"][11]["type"], "string");
    let d = vld::duration()
        .min(std::time::Duration::from_millis(1500))
        .max_secs(5)
        .json_schema();
    assert_eq!(d["anyOf"][1]["type"], "number");
    assert_eq!(d["anyOf"][1]["minimum"], 1.5);
    assert_eq!(d["anyOf"][1]["maximum"], 5);

    let js = vld::record(vld::number()).json_schema();
    assert!(js.get("propertyNames").is_none());
    assert_eq!(js, vld::record(vld::number()).ordered().json_schema());
    let js = vld::record_with_keys(vld::string().uuid(), vld::boolean())
        .min_keys(1)
        .json_schema();
    assert_eq!(
        js["propertyNames"],
        json!({"type": "string", "format": "uuid"})
    );
    assert_eq!(js["additionalProperties"], json!({"type": "boolean"}));
    assert_eq!(js["minProperties"], 1);
    let js = vld::record_with_keys(vld::enumeration(&["a", "b"]), vld::number())
        .ordered()
        .json_schema();
    assert_eq!(js["propertyNames"]["enum"], json!(["a", "b"]));

    use vld::openapi::nullable_schema;
    let n = nullable_schema(json!({"type": "string"}));
    assert_eq!(n, json!({"anyOf": [{"type": "string"}, {"type": "null"}]}));
    assert_eq!(nullable_schema(n.clone()), n);
    // A union without a `null` branch still gets wrapped.
    let union = json!({"anyOf": [{"type": "string"}, {"type": "number"}]});
    assert_eq!(
        nullable_schema(union.clone()),
        json!({"anyOf": [union, {"type": "null"}]})
    );
}

/// Given schemas whose input is wider than one JSON type (encoded bytes,
/// json values with required keys, files, decimals), schemas that accept a
/// missing key, set size hints and defaults behind wrappers
/// When their JSON Schema is emitted
/// Then: `catch` / `any` / `null` / `json_value` are not `required`, while
/// `json_value().object()` is (and parsing agrees); encoded bytes are
/// `anyOf` string / array (an array parses); `require_key` implies an object;
/// a file is a path string or an object requiring `path`; decimals are
/// string-or-number with defaults and exclusive bounds as strings; IP
/// network defaults are written out; set `min_size` is `x-minUniqueItems`.
#[test]
fn input_shapes_requiredness_and_hints() {
    let o = vld::object()
        .field("c", vld::string().catch("x".into()))
        .field("a", vld::any())
        .field("n", vld::null())
        .field("j", vld::json_value())
        .field("jo", vld::json_value().object())
        .field("s", vld::string());
    assert_eq!(o.json_schema()["required"], json!(["jo", "s"]));
    assert!(o.parse(r#"{"jo": {}, "s": "x"}"#).is_ok());

    // A union is required only when every branch is.
    let u = vld::object()
        .field("u2", vld::union(vld::string().optional(), vld::number()))
        .field(
            "u3",
            vld::union3(
                vld::string(),
                vld::number(),
                vld::boolean().with_default(true),
            ),
        )
        .field("r2", vld::union(vld::string(), vld::number()))
        .field(
            "r3",
            vld::union3(vld::string(), vld::number(), vld::boolean()),
        );
    assert_eq!(u.json_schema()["required"], json!(["r2", "r3"]));
    assert!(u.parse(r#"{"r2": "a", "r3": true}"#).is_ok());

    let js = vld::bytes().base64().json_schema();
    assert_eq!(js["anyOf"][0]["type"], "string");
    assert_eq!(js["anyOf"][1]["type"], "array");
    assert!(vld::bytes().base64().parse("[1, 2]").is_ok());
    assert_eq!(vld::bytes().json_schema()["type"], "array");

    let js = vld::json_value().require_key("a").json_schema();
    assert_eq!(js["type"], "object");
    assert_eq!(js["required"], json!(["a"]));

    assert_eq!(
        vld::set(vld::string()).min_size(2).json_schema()["x-minUniqueItems"],
        2
    );

    #[cfg(feature = "file")]
    {
        let js = vld::file().json_schema();
        assert_eq!(js["anyOf"][0], json!({"type": "string"}));
        assert_eq!(js["anyOf"][1]["required"], json!(["path"]));
    }
    #[cfg(feature = "decimal")]
    {
        assert_eq!(
            vld::decimal().json_schema()["type"],
            json!(["string", "number"])
        );
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
    #[cfg(feature = "net")]
    {
        let n = vld::ip_network().parse(r#""10.0.0.0/8""#).unwrap();
        assert_eq!(
            vld::ip_network().with_default(n).json_schema()["default"],
            "10.0.0.0/8"
        );
    }
}

/// Given date / time schemas of the date backends
/// When their JSON Schema is emitted
/// Then a time of day is RFC 3339 `partial-time` with a pattern and its
/// bounds as `formatMinimum` / `formatMaximum` (never `time`, which requires
/// an offset); a datetime accepting naive input is `anyOf` `date-time` / a
/// pattern, plain `date-time` only with `with_timezone_only`; `past` /
/// `future` are `x-past` / `x-future`; a default passes through `describe`
/// and `optional` in RFC 3339 form.
#[cfg(any(feature = "chrono", feature = "time"))]
#[test]
fn date_and_time_schemas() {
    #[cfg(feature = "chrono")]
    {
        assert_eq!(
            vld::chrono::time()
                .min("08:00:00")
                .max("18:00:00")
                .json_schema(),
            json!({"type": "string", "format": "partial-time",
                   "pattern": r"^([01]\d|2[0-3]):[0-5]\d:[0-5]\d(\.\d{1,9})?$",
                   "formatMinimum": "08:00:00", "formatMaximum": "18:00:00"})
        );
        let js = vld::chrono::time().json_schema();
        assert_eq!(js["format"], "partial-time");
        assert_eq!(
            js["pattern"],
            r"^([01]\d|2[0-3]):[0-5]\d:[0-5]\d(\.\d{1,9})?$"
        );
        let js = vld::chrono::datetime().json_schema();
        assert!(js.get("format").is_none());
        assert_eq!(js["anyOf"][0], json!({"format": "date-time"}));
        assert!(js["anyOf"][1]["pattern"].is_string());
        let js = vld::chrono::datetime().with_timezone_only().json_schema();
        assert_eq!(js["format"], "date-time");
        assert_eq!(vld::chrono::date().past().json_schema()["x-past"], true);
        assert_eq!(
            vld::chrono::datetime().future().json_schema()["x-future"],
            true
        );
    }
    #[cfg(feature = "time")]
    {
        let dt = vld::time::datetime()
            .parse_value(&json!("2024-01-02T03:04:05Z"))
            .unwrap();
        let js = vld::time::datetime()
            .describe("when")
            .with_default(dt)
            .json_schema();
        assert_eq!(js["default"], "2024-01-02T03:04:05Z");
        let js = vld::time::datetime()
            .optional()
            .with_default(Some(dt))
            .json_schema();
        assert_eq!(js["default"], "2024-01-02T03:04:05Z");
        let js = vld::time::datetime()
            .rule(vld::rules::from_fn("any", |_: &::time::OffsetDateTime| {
                true
            }))
            .with_default(dt)
            .json_schema();
        assert_eq!(js["default"], "2024-01-02T03:04:05Z");
    }
}

// ---------------------------------------------------------------------------
// Differential: vld vs the `jsonschema` crate
// ---------------------------------------------------------------------------

struct Case {
    name: &'static str,
    schema: Value,
    vld_ok: Box<dyn Fn(&Value) -> bool>,
    samples: Vec<Value>,
}

fn case<S>(name: &'static str, schema: S, samples: Vec<Value>) -> Case
where
    S: VldSchema + JsonSchema + 'static,
{
    Case {
        name,
        schema: schema.json_schema(),
        vld_ok: Box::new(move |v| schema.parse_value(v).is_ok()),
        samples,
    }
}

/// `(case, sample, vld accepts?, reason)`.
const KNOWN_DIVERGENCES: &[(&str, &str, bool, &str)] = &[
    (
        "object_strict",
        r#"{"age":null,"name":"a"}"#,
        true,
        "optional() accepts `null` (serde writes `None` as `null`) but its schema has no `null` type (TypeScript `x?: T`)",
    ),
    (
        "iso_datetime",
        r#""2024-01-15T10:30:00""#,
        true,
        "vld accepts naive datetimes; RFC 3339 `date-time` requires an offset",
    ),
    (
        "iso_time",
        r#""10:30:00""#,
        true,
        "vld `time` has no offset; RFC 3339 `time` requires one",
    ),
    (
        "url",
        r#""ftp://example.com""#,
        false,
        "vld url() is http(s) only but emits the generic `uri` format",
    ),
    (
        "url",
        r#""mailto:a@b.co""#,
        false,
        "vld url() is http(s) only but emits the generic `uri` format",
    ),
    (
        "bytes_base64",
        r#""AQ==""#,
        false,
        "a decoded byte count maps to an encoded length; padding makes shorter payloads the same length",
    ),
    (
        "set_max_unique",
        "[1, 2, 3]",
        false,
        "set size bounds count unique items: emitted as x-maxUniqueItems (not enforced)",
    ),
];

fn cases() -> Vec<Case> {
    vec![
        case(
            "string_len",
            vld::string().min(2).max(3),
            vec![
                json!("ab"),
                json!("éé"),
                json!("a"),
                json!("abcd"),
                json!(1),
            ],
        ),
        case(
            "email",
            vld::string().email(),
            vec![json!("a@b.co"), json!("nope"), json!("@x.com")],
        ),
        case(
            "uuid",
            vld::string().uuid(),
            vec![
                json!("550e8400-e29b-41d4-a716-446655440000"),
                json!("550e8400"),
            ],
        ),
        case(
            "ipv4",
            vld::string().ipv4(),
            vec![json!("1.2.3.4"), json!("256.1.1.1"), json!("01.2.3.4")],
        ),
        case(
            "ipv6",
            vld::string().ipv6(),
            vec![
                json!("::1"),
                json!("fe80::1"),
                json!("1.2.3.4"),
                json!(":::"),
            ],
        ),
        case(
            "iso_date",
            vld::string().iso_date(),
            vec![
                json!("2024-02-29"),
                json!("2023-02-29"),
                json!("2024-1-1"),
                json!("2024-02-31"),
            ],
        ),
        case(
            "iso_datetime",
            vld::string().iso_datetime(),
            vec![
                json!("2024-01-15T10:30:00Z"),
                json!("2024-01-15T10:30:00+02:00"),
                json!("2024-01-15T10:30:00"),
                json!("2024-01-15T10:30Z"),
                json!("2024-01-15"),
                json!("2024-02-31T10:00:00Z"),
            ],
        ),
        case(
            "iso_time",
            vld::string().iso_time(),
            vec![json!("10:30:00"), json!("25:00:00")],
        ),
        case(
            "url",
            vld::string().url(),
            vec![
                json!("https://example.com/a"),
                json!("ftp://example.com"),
                json!("mailto:a@b.co"),
                json!("not a url"),
            ],
        ),
        case(
            "prefix_suffix",
            vld::string().starts_with("a.b").ends_with("z"),
            vec![json!("a.bxz"), json!("axbz"), json!("a.b")],
        ),
        case(
            "number_bounds",
            vld::number().min(0.0).lt(10.0),
            vec![json!(0), json!(9.99), json!(10), json!(-0.1), json!("1")],
        ),
        case(
            "int_exclusive",
            vld::number().int().positive().max(5),
            vec![json!(1), json!(5), json!(0), json!(6), json!(1.5)],
        ),
        case(
            "int_safe",
            vld::number().int().safe(),
            vec![json!(9007199254740991_i64), json!(9007199254740992_i64)],
        ),
        case(
            "multiple_of_float",
            vld::number().multiple_of(0.1),
            vec![json!(0.5), json!(0.3), json!(0.35)],
        ),
        case(
            "multiple_of_int",
            vld::number().int().multiple_of(3),
            vec![json!(9), json!(10)],
        ),
        case(
            "enum",
            vld::enumeration(&["a", "é"]),
            vec![json!("a"), json!("é"), json!("b"), json!(1)],
        ),
        case(
            "literal",
            vld::literal(5i64),
            vec![json!(5), json!(5.0), json!(6), json!("5")],
        ),
        case(
            "array",
            vld::array(vld::number().int())
                .min_len(1)
                .max_len(2)
                .unique(),
            vec![
                json!([1]),
                json!([1, 2]),
                json!([]),
                json!([1, 2, 3]),
                json!([1, 1]),
                json!(["x"]),
            ],
        ),
        case(
            "array_contains",
            vld::array(vld::number()).contains(5),
            vec![json!([1, 5]), json!([1, 2]), json!([])],
        ),
        case(
            "tuple",
            (vld::string(), vld::number()),
            vec![
                json!(["a", 1]),
                json!(["a"]),
                json!(["a", 1, 2]),
                json!([1, "a"]),
            ],
        ),
        case(
            "record",
            vld::record(vld::number().positive())
                .min_keys(1)
                .max_keys(2),
            vec![
                json!({"a": 1}),
                json!({}),
                json!({"a": -1}),
                json!({"a": 1, "b": 2, "c": 3}),
            ],
        ),
        case(
            "set_max_unique",
            vld::set(vld::number().int()).max_size(2),
            vec![json!([1, 2]), json!([1, 1, 2]), json!([1, 2, 3])],
        ),
        case(
            "object_strict",
            vld::object()
                .field("name", vld::string().min(1))
                .field("age", vld::number().int().optional())
                .strict(),
            vec![
                json!({"name": "a"}),
                json!({"name": "a", "age": null}),
                json!({"name": "a", "age": 3}),
                json!({"name": ""}),
                json!({}),
                json!({"name": "a", "x": 1}),
                json!({"name": "a", "age": "x"}),
            ],
        ),
        case(
            "object_catchall",
            vld::object()
                .field("a", vld::string())
                .catchall(vld::number()),
            vec![json!({"a": "x", "b": 1}), json!({"a": "x", "b": "y"})],
        ),
        case(
            "nullable",
            vld::string().min(2).nullable(),
            vec![json!(null), json!("ab"), json!("a"), json!(1)],
        ),
        case(
            "union",
            vld::union(vld::string().min(3), vld::number().min(10.0)),
            vec![json!("abc"), json!(10), json!("ab"), json!(3), json!(true)],
        ),
        case(
            "intersection",
            vld::intersection(vld::string().min(2), vld::string().max(3)),
            vec![json!("ab"), json!("a"), json!("abcd")],
        ),
        case(
            "discriminated",
            vld::discriminated_union("type")
                .variant_str(
                    "a",
                    vld::object()
                        .field("type", vld::literal("a"))
                        .field("n", vld::number()),
                )
                .variant_str("b", vld::object().field("type", vld::literal("b"))),
            vec![
                json!({"type": "a", "n": 1}),
                json!({"type": "a"}),
                json!({"type": "b"}),
                json!({"type": "c"}),
                json!({}),
            ],
        ),
        case(
            "bytes_base64",
            vld::bytes().base64().len(3),
            vec![json!("AQID"), json!("AQ==")],
        ),
    ]
}

/// Given hand-picked schemas (formats, bounds, multiples, enums, literals,
/// collections, objects, unions, discriminated unions, bytes) and samples on
/// both sides of each rule
/// When vld and the `jsonschema` crate (draft 2020-12, formats asserted)
/// judge each sample against the schema vld emits
/// Then they agree, except exactly the listed known divergences (a listed
/// pair that stops diverging fails too, so the list stays exact).
#[test]
fn vld_and_jsonschema_agree() {
    let mut problems = Vec::new();
    let mut seen_known = Vec::new();
    for c in cases() {
        let validator = jsonschema::draft202012::options()
            .should_validate_formats(true)
            .build(&c.schema)
            .unwrap_or_else(|e| panic!("{}: emitted schema is invalid: {e}\n{}", c.name, c.schema));
        for sample in &c.samples {
            let vld = (c.vld_ok)(sample);
            let js = validator.is_valid(sample);
            let known = KNOWN_DIVERGENCES.iter().find(|(n, s, _, _)| {
                *n == c.name && serde_json::from_str::<Value>(s).unwrap() == *sample
            });
            match (vld == js, known) {
                (true, None) => {}
                (false, Some(&(_, _, expected_vld, _))) if expected_vld == vld => {
                    seen_known.push((c.name, sample.clone()));
                }
                (false, _) => problems.push(format!(
                    "{}: {sample} vld={vld} jsonschema={js}\n    schema: {}",
                    c.name, c.schema
                )),
                (true, Some(_)) => problems.push(format!(
                    "{}: {sample} is listed as a known divergence but both agree ({vld})",
                    c.name
                )),
            }
        }
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
    assert_eq!(seen_known.len(), KNOWN_DIVERGENCES.len(), "{seen_known:?}");
}
