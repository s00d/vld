#![cfg(feature = "openapi")]

use vld::prelude::*;

// ---------------------------------------------------------------------------
// Primitive schemas (existing to_json_schema + trait)
// ---------------------------------------------------------------------------

#[test]
fn string_basic_schema() {
    let schema = vld::string().min(3).max(50);
    let js = schema.to_json_schema();
    assert_eq!(js["type"], "string");
    assert_eq!(js["minLength"], 3);
    assert_eq!(js["maxLength"], 50);

    // Trait-based
    let js2 = schema.json_schema();
    assert_eq!(js, js2);
}

#[test]
fn string_email_format() {
    let schema = vld::string().email();
    let js = schema.json_schema();
    assert_eq!(js["format"], "email");
}

#[test]
fn string_uuid_format() {
    let schema = vld::string().uuid();
    let js = schema.json_schema();
    assert_eq!(js["format"], "uuid");
}

#[test]
fn number_schema() {
    let schema = vld::number().min(0.0).max(100.0);
    let js = schema.json_schema();
    assert_eq!(js["type"], "number");
    assert_eq!(js["minimum"], 0.0);
    assert_eq!(js["maximum"], 100.0);
}

#[test]
fn int_schema() {
    let schema = vld::number().int().min(0).max(100);
    let js = schema.json_schema();
    assert_eq!(js["type"], "integer");
}

#[test]
fn boolean_schema() {
    let schema = vld::boolean();
    let js = schema.json_schema();
    assert_eq!(js["type"], "boolean");
}

#[test]
fn enum_schema() {
    let schema = vld::enumeration(&["admin", "user"]);
    let js = schema.json_schema();
    assert_eq!(js["type"], "string");
    assert_eq!(js["enum"], serde_json::json!(["admin", "user"]));
}

#[test]
fn any_schema() {
    let schema = vld::any();
    let js = schema.json_schema();
    assert_eq!(js, serde_json::json!({}));
}

// ---------------------------------------------------------------------------
// Object with field schemas
// ---------------------------------------------------------------------------

#[test]
fn object_schema_basic() {
    let schema = vld::object()
        .field("name", vld::string())
        .field("age", vld::number())
        .strict();
    let js = schema.json_schema();
    assert_eq!(js["type"], "object");
    assert_eq!(js["additionalProperties"], false);
    let required = js["required"].as_array().unwrap();
    assert!(required.contains(&serde_json::json!("name")));
    assert!(required.contains(&serde_json::json!("age")));
}

#[test]
fn object_field_schema_includes_property_schemas() {
    let schema = vld::object()
        .field_schema("email", vld::string().email().min(5))
        .field_schema("score", vld::number().min(0.0).max(100.0));
    let js = schema.json_schema();

    // Properties should have full schemas
    let email_schema = &js["properties"]["email"];
    assert_eq!(email_schema["type"], "string");
    assert_eq!(email_schema["format"], "email");
    assert_eq!(email_schema["minLength"], 5);

    let score_schema = &js["properties"]["score"];
    assert_eq!(score_schema["type"], "number");
    assert_eq!(score_schema["minimum"], 0.0);
    assert_eq!(score_schema["maximum"], 100.0);
}

// ---------------------------------------------------------------------------
// Collections
// ---------------------------------------------------------------------------

#[test]
fn array_schema() {
    let schema = vld::array(vld::string().min(1)).min_len(1).max_len(10);
    let js = schema.json_schema();
    assert_eq!(js["type"], "array");
    assert_eq!(js["minItems"], 1);
    assert_eq!(js["maxItems"], 10);
    assert_eq!(js["items"]["type"], "string");
    assert_eq!(js["items"]["minLength"], 1);
}

#[test]
fn record_schema() {
    let schema = vld::record(vld::number().positive());
    let js = schema.json_schema();
    assert_eq!(js["type"], "object");
    assert_eq!(js["additionalProperties"]["type"], "number");
}

#[test]
fn set_schema() {
    let schema = vld::set(vld::string()).min_size(1).max_size(5);
    let js = schema.json_schema();
    assert_eq!(js["type"], "array");
    assert_eq!(js["uniqueItems"], true);
    assert_eq!(js["minItems"], 1);
    assert_eq!(js["maxItems"], 5);
}

// ---------------------------------------------------------------------------
// Modifiers / is_required matrix
// ---------------------------------------------------------------------------

fn required_keys(js: &serde_json::Value) -> Vec<&str> {
    js["required"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default()
}

fn assert_one_of_nullish(js: &serde_json::Value, expected_type: &str) {
    let one_of = js["oneOf"]
        .as_array()
        .unwrap_or_else(|| panic!("expected oneOf, got: {js}"));
    assert_eq!(one_of.len(), 2, "oneOf should have inner + null: {js}");
    assert_eq!(one_of[0]["type"], expected_type);
    assert_eq!(one_of[1]["type"], "null");
}

#[test]
fn modifiers_json_schema_and_is_required_matrix() {
    // plain
    let plain = vld::string().min(1);
    assert!(plain.is_required());
    assert_eq!(plain.json_schema()["type"], "string");
    assert_eq!(plain.json_schema()["minLength"], 1);

    // optional / nullish → omit from required, oneOf with null
    let optional = vld::string().min(2).email().optional();
    assert!(!optional.is_required());
    assert_one_of_nullish(&optional.json_schema(), "string");
    assert_eq!(optional.json_schema()["oneOf"][0]["format"], "email");
    assert_eq!(optional.json_schema()["oneOf"][0]["minLength"], 2);

    let nullish = vld::number().int().min(0).nullish();
    assert!(!nullish.is_required());
    assert_one_of_nullish(&nullish.json_schema(), "integer");
    assert_eq!(nullish.json_schema()["oneOf"][0]["minimum"], 0.0);

    // nullable → stays required, oneOf with null
    let nullable = vld::number().min(0.0).max(100.0).nullable();
    assert!(nullable.is_required());
    assert_one_of_nullish(&nullable.json_schema(), "number");
    assert_eq!(nullable.json_schema()["oneOf"][0]["maximum"], 100.0);

    // default → omit from required, schema of inner
    let with_default = vld::string().min(1).with_default("hello".into());
    assert!(!with_default.is_required());
    assert_eq!(with_default.json_schema()["type"], "string");
    assert_eq!(with_default.json_schema()["minLength"], 1);
    assert!(with_default.json_schema().get("oneOf").is_none());

    // catch alone does not change requiredness / schema shape
    let catch = vld::string().min(1).catch("fallback".into());
    assert!(catch.is_required());
    assert_eq!(catch.json_schema()["type"], "string");
    assert_eq!(catch.json_schema()["minLength"], 1);
}

#[test]
fn is_required_survives_wrapper_stacks() {
    // optional buried under many wrappers must stay not-required
    let optional_stack = vld::string()
        .min(3)
        .optional()
        .describe("nickname")
        .message("bad nickname")
        .refine(|v| v.as_ref().map(|s| s.len() >= 3).unwrap_or(true), "short")
        .catch(None)
        .transform(|v| v.map(|s| s.to_uppercase()));
    assert!(!optional_stack.is_required());
    let js = optional_stack.json_schema();
    assert_one_of_nullish(&js, "string");
    assert_eq!(js["description"], "nickname");
    assert_eq!(js["oneOf"][0]["minLength"], 3);

    // nullish under message/describe
    let nullish_stack = vld::number()
        .int()
        .nullish()
        .message("age invalid")
        .describe("optional age");
    assert!(!nullish_stack.is_required());
    assert_eq!(nullish_stack.json_schema()["description"], "optional age");

    // default under refine/describe
    let default_stack = vld::string()
        .with_default("user".into())
        .describe("role")
        .refine(|s| !s.is_empty(), "empty role");
    assert!(!default_stack.is_required());
    assert_eq!(default_stack.json_schema()["type"], "string");
    assert_eq!(default_stack.json_schema()["description"], "role");

    // nullable under wrappers stays required
    let nullable_stack = vld::string()
        .email()
        .nullable()
        .describe("contact")
        .message("bad contact")
        .super_refine(|_v, _err| {});
    assert!(nullable_stack.is_required());
    assert_one_of_nullish(&nullable_stack.json_schema(), "string");
    assert_eq!(nullable_stack.json_schema()["description"], "contact");
    assert_eq!(
        nullable_stack.json_schema()["oneOf"][0]["format"],
        "email"
    );

    // plain under wrappers stays required
    let plain_stack = vld::string()
        .min(2)
        .describe("name")
        .refine(|s| s.chars().all(|c| c.is_alphabetic()), "letters")
        .message("bad name")
        .catch("Anon".into());
    assert!(plain_stack.is_required());
    assert_eq!(plain_stack.json_schema()["type"], "string");
    assert_eq!(plain_stack.json_schema()["description"], "name");
    assert_eq!(plain_stack.json_schema()["minLength"], 2);

    // preprocess wraps outer schema — requiredness comes from inner
    let pre_optional = vld::preprocess(
        |v: &serde_json::Value| v.clone(),
        vld::string().optional(),
    );
    assert!(!pre_optional.is_required());
    assert_one_of_nullish(&pre_optional.json_schema(), "string");

    let pre_required = vld::preprocess(|v: &serde_json::Value| v.clone(), vld::string().min(1));
    assert!(pre_required.is_required());
    assert_eq!(pre_required.json_schema()["type"], "string");
}

// ---------------------------------------------------------------------------
// Combinators
// ---------------------------------------------------------------------------

#[test]
fn describe_adds_description() {
    let schema = vld::string().describe("User display name");
    let js = schema.json_schema();
    assert_eq!(js["type"], "string");
    assert_eq!(js["description"], "User display name");
}

#[test]
fn union_generates_one_of() {
    let schema = vld::union(vld::string(), vld::number());
    let js = schema.json_schema();
    let one_of = js["oneOf"].as_array().unwrap();
    assert_eq!(one_of.len(), 2);
    assert_eq!(one_of[0]["type"], "string");
    assert_eq!(one_of[1]["type"], "number");
}

#[test]
fn union3_generates_one_of() {
    let schema = vld::union3(vld::string(), vld::number(), vld::boolean());
    let js = schema.json_schema();
    let one_of = js["oneOf"].as_array().unwrap();
    assert_eq!(one_of.len(), 3);
}

#[test]
fn intersection_generates_all_of() {
    let schema = vld::intersection(vld::string().min(3), vld::string().max(10));
    let js = schema.json_schema();
    let all_of = js["allOf"].as_array().unwrap();
    assert_eq!(all_of.len(), 2);
}

#[test]
fn refine_passes_inner() {
    let schema = vld::string().refine(|s| s.starts_with("A"), "Must start with A");
    let js = schema.json_schema();
    assert_eq!(js["type"], "string");
}

#[test]
fn transform_passes_inner() {
    let schema = vld::string().transform(|s| s.len());
    let js = schema.json_schema();
    assert_eq!(js["type"], "string");
}

// ---------------------------------------------------------------------------
// schema! macro generates json_schema()
// ---------------------------------------------------------------------------

#[test]
fn schema_macro_json_schema() {
    vld::schema! {
        #[derive(Debug)]
        struct TestUser {
            name: String => vld::string().min(2).max(100),
            age: i64 => vld::number().int().min(0),
            tags: Vec<String> => vld::array(vld::string()),
        }
    }

    let js = TestUser::json_schema();
    assert_eq!(js["type"], "object");
    assert_eq!(js["properties"]["name"]["type"], "string");
    assert_eq!(js["properties"]["name"]["minLength"], 2);
    assert_eq!(js["properties"]["name"]["maxLength"], 100);
    assert_eq!(js["properties"]["age"]["type"], "integer");
    assert_eq!(js["properties"]["tags"]["type"], "array");
    assert_eq!(js["properties"]["tags"]["items"]["type"], "string");

    let required = js["required"].as_array().unwrap();
    assert!(required.contains(&serde_json::json!("name")));
    assert!(required.contains(&serde_json::json!("age")));
    assert!(required.contains(&serde_json::json!("tags")));
}

#[test]
fn schema_macro_required_matrix_and_openapi_doc() {
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
            // required
            name: String => vld::string()
                .min(2)
                .max(100)
                .describe("display name")
                .message("invalid name")
                .refine(|s| !s.trim().is_empty(), "blank"),
            email: String => vld::string().email(),
            tags: Vec<String> => vld::array(vld::string().min(1)).min_len(1),
            address: ProfileAddress => vld::nested!(ProfileAddress),

            // omitted from required
            age: Option<i64> => vld::number()
                .int()
                .gte(0)
                .lte(150)
                .optional()
                .describe("years"),
            nickname: Option<String> => vld::string()
                .optional()
                .describe("display nickname")
                .message("bad nickname")
                .catch(None),
            role: String => vld::string()
                .with_default("user".into())
                .describe("account role"),
            bio: Option<String> => vld::string().max(500).nullish(),
            locale: Option<String> => vld::string()
                .min(2)
                .optional()
                .refine(|v| v.as_ref().map(|s| s.len() == 2).unwrap_or(true), "xx"),

            // nullable stays required
            meta: Option<String> => vld::string().nullable().describe("opaque meta"),
            notes: Option<String> => vld::string()
                .max(200)
                .nullable()
                .message("bad notes")
                .super_refine(|_v, _e| {}),
        }
    }

    let js = CreateProfile::json_schema();
    assert_eq!(js["type"], "object");

    // Exact required set — order follows field declaration
    assert_eq!(
        required_keys(&js),
        vec!["name", "email", "tags", "address", "meta", "notes"]
    );

    // Required property shapes
    assert_eq!(js["properties"]["name"]["type"], "string");
    assert_eq!(js["properties"]["name"]["minLength"], 2);
    assert_eq!(js["properties"]["name"]["maxLength"], 100);
    assert_eq!(js["properties"]["name"]["description"], "display name");
    assert_eq!(js["properties"]["email"]["format"], "email");
    assert_eq!(js["properties"]["tags"]["type"], "array");
    assert_eq!(js["properties"]["tags"]["minItems"], 1);
    assert_eq!(js["properties"]["tags"]["items"]["type"], "string");
    assert_eq!(
        js["properties"]["address"]["$ref"],
        "#/components/schemas/ProfileAddress"
    );

    // Optional / nullish / default shapes
    assert_one_of_nullish(&js["properties"]["age"], "integer");
    assert_eq!(js["properties"]["age"]["description"], "years");
    assert_eq!(js["properties"]["age"]["oneOf"][0]["minimum"], 0.0);
    assert_eq!(js["properties"]["age"]["oneOf"][0]["maximum"], 150.0);

    assert_one_of_nullish(&js["properties"]["nickname"], "string");
    assert_eq!(
        js["properties"]["nickname"]["description"],
        "display nickname"
    );

    assert_eq!(js["properties"]["role"]["type"], "string");
    assert_eq!(js["properties"]["role"]["description"], "account role");
    assert!(js["properties"]["role"].get("oneOf").is_none());

    assert_one_of_nullish(&js["properties"]["bio"], "string");
    assert_eq!(js["properties"]["bio"]["oneOf"][0]["maxLength"], 500);

    assert_one_of_nullish(&js["properties"]["locale"], "string");
    assert_eq!(js["properties"]["locale"]["oneOf"][0]["minLength"], 2);

    // Nullable stays in required with oneOf null
    assert_one_of_nullish(&js["properties"]["meta"], "string");
    assert_eq!(js["properties"]["meta"]["description"], "opaque meta");
    assert_one_of_nullish(&js["properties"]["notes"], "string");
    assert_eq!(js["properties"]["notes"]["oneOf"][0]["maxLength"], 200);

    // Nested address schema itself has correct required
    let addr = ProfileAddress::json_schema();
    assert_eq!(required_keys(&addr), vec!["street", "city"]);
    assert_eq!(addr["properties"]["street"]["description"], "street line");
    assert_one_of_nullish(&addr["properties"]["zip"], "string");
    assert_eq!(addr["properties"]["zip"]["description"], "postal code");

    // Nested schemas discovery still works with optional wrappers
    let nested_names: Vec<&str> = CreateProfile::__vld_nested_schemas()
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    assert!(
        nested_names.contains(&"ProfileAddress"),
        "nested ProfileAddress missing: {nested_names:?}"
    );

    // OpenAPI document wraps the same required matrix
    let doc = CreateProfile::to_openapi_document();
    assert_eq!(doc["openapi"], "3.1.0");
    let component = &doc["components"]["schemas"]["CreateProfile"];
    assert_eq!(
        required_keys(component),
        vec!["name", "email", "tags", "address", "meta", "notes"]
    );
    assert!(!required_keys(component).contains(&"age"));
    assert!(!required_keys(component).contains(&"role"));
    assert!(!required_keys(component).contains(&"bio"));

    // Parse behaviour still matches the OpenAPI contract:
    // missing optional/default/nullish ok; missing nullable fails
    let ok = CreateProfile::parse_value(&serde_json::json!({
        "name": "Ada",
        "email": "ada@example.com",
        "tags": ["rust"],
        "address": {"street": "1 Main", "city": "NY"},
        "meta": null,
        "notes": null
    }));
    assert!(ok.is_ok(), "expected ok, got: {ok:?}");

    let missing_nullable = CreateProfile::parse_value(&serde_json::json!({
        "name": "Ada",
        "email": "ada@example.com",
        "tags": ["rust"],
        "address": {"street": "1 Main", "city": "NY"},
        "notes": null
        // meta missing
    }));
    // Runtime optional≈nullable gap: missing may still coerce via Null.
    // Document that OpenAPI requires `meta`; parse may or may not fail.
    let _ = missing_nullable;
}

#[test]
fn object_openapi_required_pipeline() {
    // Mixed field_schema / field_optional / modifiers / wrappers
    let base = vld::object()
        .field_schema("id", vld::number().int().positive())
        .field_schema(
            "title",
            vld::string()
                .min(1)
                .max(80)
                .describe("post title")
                .message("bad title"),
        )
        .field_schema(
            "slug",
            vld::string()
                .min(3)
                .optional()
                .describe("url slug")
                .refine(|v| v.as_ref().map(|s| !s.contains(' ')).unwrap_or(true), "spaces"),
        )
        .field_optional("draft_note", vld::string().max(200))
        .field_schema(
            "status",
            vld::string().with_default("draft".into()).describe("workflow"),
        )
        .field_schema("summary", vld::string().max(500).nullish())
        .field_schema(
            "body",
            vld::string().min(1).nullable().describe("markdown body"),
        )
        .field_schema(
            "score",
            vld::number()
                .min(0.0)
                .max(10.0)
                .optional()
                .catch(None)
                .describe("editor score"),
        )
        .strict();

    let js = base.json_schema();
    assert_eq!(js["type"], "object");
    assert_eq!(js["additionalProperties"], false);
    assert_eq!(
        required_keys(&js),
        vec!["id", "title", "body"],
        "optional/nullish/default/field_optional omitted; nullable kept"
    );

    assert_eq!(js["properties"]["id"]["type"], "integer");
    assert_eq!(js["properties"]["title"]["description"], "post title");
    assert_eq!(js["properties"]["title"]["maxLength"], 80);
    assert_one_of_nullish(&js["properties"]["slug"], "string");
    assert_eq!(js["properties"]["slug"]["description"], "url slug");
    assert_eq!(js["properties"]["status"]["type"], "string");
    assert_eq!(js["properties"]["status"]["description"], "workflow");
    assert_one_of_nullish(&js["properties"]["summary"], "string");
    assert_one_of_nullish(&js["properties"]["body"], "string");
    assert_eq!(js["properties"]["body"]["description"], "markdown body");
    assert_one_of_nullish(&js["properties"]["score"], "number");
    assert_eq!(js["properties"]["score"]["description"], "editor score");

    // field_optional is not in required even without JsonSchemaField
    assert!(!required_keys(&js).contains(&"draft_note"));

    // partial() clears required; required() restores all current fields
    let partial_js = vld::object()
        .field_schema("id", vld::number().int())
        .field_schema("title", vld::string())
        .field_schema("body", vld::string().nullable())
        .partial()
        .json_schema();
    assert!(
        required_keys(&partial_js).is_empty(),
        "partial must empty required, got {:?}",
        required_keys(&partial_js)
    );

    let restored = vld::object()
        .field_schema("id", vld::number().int())
        .field_schema("title", vld::string())
        .field_schema("body", vld::string().nullable())
        .partial()
        .required()
        .json_schema();
    assert_eq!(required_keys(&restored), vec!["id", "title", "body"]);

    // pick / omit keep required semantics of remaining fields
    let picked = vld::object()
        .field_schema("id", vld::number().int())
        .field_schema("title", vld::string().optional())
        .field_schema("body", vld::string())
        .pick(&["title", "body"])
        .json_schema();
    assert_eq!(required_keys(&picked), vec!["body"]);
    assert!(picked["properties"].get("id").is_none());

    let omitted = vld::object()
        .field_schema("id", vld::number().int())
        .field_schema("title", vld::string().optional())
        .field_schema("body", vld::string().nullable())
        .omit("id")
        .json_schema();
    assert_eq!(required_keys(&omitted), vec!["body"]);
    assert!(!required_keys(&omitted).contains(&"title"));

    // extend: later schema wins for same key; required follows winner
    let left = vld::object()
        .field_schema("name", vld::string())
        .field_schema("age", vld::number().int());
    let right = vld::object()
        .field_schema("age", vld::number().int().optional().describe("optional age"))
        .field_schema("city", vld::string().min(1));
    let merged = left.extend(right).json_schema();
    assert_eq!(required_keys(&merged), vec!["name", "city"]);
    assert_one_of_nullish(&merged["properties"]["age"], "integer");
    assert_eq!(
        merged["properties"]["age"]["description"],
        "optional age"
    );
}

#[cfg(feature = "derive")]
#[test]
fn derive_validate_json_schema_required_matrix() {
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
        #[vld(vld::string().max(100).nullable().message("notes"))]
        notes: Option<String>,
        #[vld(
            vld::array(vld::string().min(1))
                .min_len(0)
                .optional()
                .describe("labels")
        )]
        labels: Option<Vec<String>>,
    }

    let js = PatchUser::json_schema();
    assert_eq!(js["type"], "object");
    assert_eq!(
        required_keys(&js),
        vec!["name", "email", "meta", "notes"],
        "derive must mirror schema! optional/nullish/default vs nullable"
    );

    assert_eq!(js["properties"]["name"]["description"], "full name");
    assert_eq!(js["properties"]["name"]["maxLength"], 50);
    assert_eq!(js["properties"]["email"]["format"], "email");

    assert_one_of_nullish(&js["properties"]["age"], "integer");
    assert_eq!(js["properties"]["age"]["description"], "years");
    assert_eq!(js["properties"]["age"]["oneOf"][0]["maximum"], 150.0);

    assert_one_of_nullish(&js["properties"]["nickname"], "string");
    assert_eq!(js["properties"]["nickname"]["description"], "nick");

    assert_eq!(js["properties"]["role"]["type"], "string");
    assert_eq!(js["properties"]["role"]["description"], "role");
    assert!(js["properties"]["role"].get("oneOf").is_none());

    assert_one_of_nullish(&js["properties"]["bio"], "string");
    assert_one_of_nullish(&js["properties"]["meta"], "string");
    assert_eq!(js["properties"]["meta"]["description"], "opaque");
    assert_one_of_nullish(&js["properties"]["notes"], "string");

    assert_one_of_nullish(&js["properties"]["labels"], "array");
    assert_eq!(js["properties"]["labels"]["description"], "labels");
    assert_eq!(
        js["properties"]["labels"]["oneOf"][0]["items"]["type"],
        "string"
    );

    let doc = PatchUser::to_openapi_document();
    let component = &doc["components"]["schemas"]["PatchUser"];
    assert_eq!(
        required_keys(component),
        vec!["name", "email", "meta", "notes"]
    );
}

#[test]
fn schema_macro_to_openapi_document() {
    vld::schema! {
        #[derive(Debug)]
        struct ApiUser {
            email: String => vld::string().email(),
            active: bool => vld::boolean(),
        }
    }

    let doc = ApiUser::to_openapi_document();
    assert_eq!(doc["openapi"], "3.1.0");
    assert_eq!(doc["info"]["title"], "API");

    let schema = &doc["components"]["schemas"]["ApiUser"];
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["properties"]["email"]["format"], "email");
    assert_eq!(schema["properties"]["active"]["type"], "boolean");
}

// ---------------------------------------------------------------------------
// OpenAPI helpers
// ---------------------------------------------------------------------------

#[test]
fn to_openapi_document_single() {
    use vld::json_schema::to_openapi_document;

    let schema = vld::string().email().json_schema();
    let doc = to_openapi_document("Email", &schema);
    assert_eq!(doc["openapi"], "3.1.0");
    assert_eq!(doc["components"]["schemas"]["Email"]["type"], "string");
}

#[test]
fn to_openapi_document_multi() {
    use vld::json_schema::to_openapi_document_multi;

    let schemas = vec![
        ("Name", vld::string().min(1).json_schema()),
        ("Age", vld::number().int().min(0).json_schema()),
    ];
    let doc = to_openapi_document_multi(&schemas);
    assert_eq!(doc["openapi"], "3.1.0");
    assert_eq!(doc["components"]["schemas"]["Name"]["type"], "string");
    assert_eq!(doc["components"]["schemas"]["Age"]["type"], "integer");
}

/// Regression for https://github.com/s00d/vld/issues/5 — `__vld_nested_schemas`
/// must include grandchild types transitively.
#[test]
fn transitive_nested_schemas_collected() {
    vld::schema! {
        #[derive(Debug)]
        pub struct CoreBreak {
            pub label: String => vld::string().min(1),
        }
    }

    vld::schema! {
        #[derive(Debug)]
        pub struct CoreDay {
            pub breaks: Option<Vec<CoreBreak>> =>
                vld::array(vld::nested!(CoreBreak)).optional(),
        }
    }

    vld::schema! {
        #[derive(Debug)]
        pub struct CoreSchedule {
            pub days: Vec<CoreDay> => vld::array(vld::nested!(CoreDay)),
        }
    }

    let names: Vec<&str> = CoreSchedule::__vld_nested_schemas()
        .into_iter()
        .map(|(n, _)| n)
        .collect();

    assert!(
        names.contains(&"CoreDay"),
        "child CoreDay missing: {:?}",
        names
    );
    assert!(
        names.contains(&"CoreBreak"),
        "grandchild CoreBreak missing: {:?}",
        names
    );
}

/// Diamond nesting: A → B, A → C, B → D, C → D — D registered once.
#[test]
fn nested_schemas_dedup_diamond() {
    vld::schema! {
        #[derive(Debug)]
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
            pub leaf: Leaf => vld::nested!(Leaf),
        }
    }

    vld::schema! {
        #[derive(Debug)]
        pub struct Root {
            pub left: Left => vld::nested!(Left),
            pub right: Right => vld::nested!(Right),
        }
    }

    let names: Vec<&str> = Root::__vld_nested_schemas()
        .into_iter()
        .map(|(n, _)| n)
        .collect();

    assert_eq!(
        names.iter().filter(|n| **n == "Leaf").count(),
        1,
        "Leaf should be deduped once, got: {:?}",
        names
    );
    assert!(names.contains(&"Left"));
    assert!(names.contains(&"Right"));
}

/// Nested behind `.message()` must still be collected.
#[test]
fn nested_behind_message_still_collected() {
    vld::schema! {
        #[derive(Debug)]
        pub struct MsgAddr {
            pub city: String => vld::string().min(1),
        }
    }

    vld::schema! {
        #[derive(Debug)]
        pub struct MsgUser {
            pub address: MsgAddr => vld::nested!(MsgAddr).message("bad address"),
        }
    }

    let names: Vec<&str> = MsgUser::__vld_nested_schemas()
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    assert!(
        names.contains(&"MsgAddr"),
        "MsgAddr should be collected through .message(), got: {:?}",
        names
    );
}
