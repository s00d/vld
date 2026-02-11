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
// Modifiers
// ---------------------------------------------------------------------------

#[test]
fn optional_schema() {
    let schema = vld::string().optional();
    let js = schema.json_schema();
    let one_of = js["oneOf"].as_array().unwrap();
    assert_eq!(one_of.len(), 2);
    assert_eq!(one_of[0]["type"], "string");
    assert_eq!(one_of[1]["type"], "null");
}

#[test]
fn nullable_schema() {
    let schema = vld::number().nullable();
    let js = schema.json_schema();
    let one_of = js["oneOf"].as_array().unwrap();
    assert_eq!(one_of[0]["type"], "number");
    assert_eq!(one_of[1]["type"], "null");
}

#[test]
fn default_schema_passes_through() {
    let schema = vld::string().min(1).with_default("hello".into());
    let js = schema.json_schema();
    assert_eq!(js["type"], "string");
    assert_eq!(js["minLength"], 1);
}

#[test]
fn catch_schema_passes_through() {
    let schema = vld::string().min(1).catch("fallback".into());
    let js = schema.json_schema();
    assert_eq!(js["type"], "string");
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
