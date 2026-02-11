//! Tests that the core validation library works with `--no-default-features`.
//!
//! These tests must NOT rely on:
//!   - `std` feature (no Path/PathBuf VldInput, no save_to_file)
//!   - `serialize` feature (no serde::Serialize bounds, no validate()/is_valid())
//!   - `openapi` feature (no JsonSchema, no json_schema()/to_openapi_document())
//!   - `chrono` feature (no ZDate/ZDateTime)
//!   - `regex` feature (no .regex())
//!   - `derive` feature (no #[derive(Validate)])
//!
//! Run with: cargo test -p vld --test no_default_features --no-default-features

use vld::prelude::*;

// -----------------------------------------------------------------------
// Core parsing from &str, String, &[u8], serde_json::Value
// -----------------------------------------------------------------------

#[test]
fn parse_string_from_str() {
    let schema = vld::string().min(2).max(50);
    let result = schema.parse(r#""hello""#).unwrap();
    assert_eq!(result, "hello");
}

#[test]
fn parse_string_from_string() {
    let input = String::from(r#""world""#);
    let schema = vld::string();
    let result = schema.parse(&input).unwrap();
    assert_eq!(result, "world");
}

#[test]
fn parse_from_bytes() {
    let input = b"42";
    let schema = vld::number();
    let result = schema.parse(input.as_ref()).unwrap();
    assert!((result - 42.0).abs() < f64::EPSILON);
}

#[test]
fn parse_from_value() {
    let val = serde_json::json!(true);
    let schema = vld::boolean();
    let result = schema.parse_value(&val).unwrap();
    assert!(result);
}

// -----------------------------------------------------------------------
// String validators
// -----------------------------------------------------------------------

#[test]
fn string_min_max() {
    let schema = vld::string().min(3).max(10);
    assert!(schema.parse(r#""abc""#).is_ok());
    assert!(schema.parse(r#""ab""#).is_err());
    assert!(schema.parse(r#""abcdefghijk""#).is_err());
}

#[test]
fn string_email() {
    let schema = vld::string().email();
    assert!(schema.parse(r#""user@example.com""#).is_ok());
    assert!(schema.parse(r#""nope""#).is_err());
}

#[test]
fn string_uuid() {
    let schema = vld::string().uuid();
    assert!(schema
        .parse(r#""550e8400-e29b-41d4-a716-446655440000""#)
        .is_ok());
    assert!(schema.parse(r#""not-a-uuid""#).is_err());
}

#[test]
fn string_url() {
    let schema = vld::string().url();
    assert!(schema.parse(r#""https://example.com""#).is_ok());
    assert!(schema.parse(r#""notaurl""#).is_err());
}

#[test]
fn string_starts_ends_contains() {
    assert!(vld::string()
        .starts_with("foo")
        .parse(r#""foobar""#)
        .is_ok());
    assert!(vld::string().ends_with("bar").parse(r#""foobar""#).is_ok());
    assert!(vld::string().contains("oba").parse(r#""foobar""#).is_ok());
}

#[test]
fn string_trim() {
    let schema = vld::string().trim().min(3);
    assert!(schema.parse(r#""  hello  ""#).is_ok());
    assert!(schema.parse(r#""  ab  ""#).is_err());
}

#[test]
fn string_coerce() {
    let schema = vld::string().coerce();
    assert_eq!(schema.parse("42").unwrap(), "42");
    assert_eq!(schema.parse("true").unwrap(), "true");
}

#[test]
fn string_non_empty() {
    let schema = vld::string().non_empty();
    assert!(schema.parse(r#""""#).is_err());
    assert!(schema.parse(r#""a""#).is_ok());
}

// -----------------------------------------------------------------------
// Number validators
// -----------------------------------------------------------------------

#[test]
fn number_min_max() {
    let schema = vld::number().min(0.0).max(100.0);
    assert!(schema.parse("50").is_ok());
    assert!(schema.parse("-1").is_err());
    assert!(schema.parse("101").is_err());
}

#[test]
fn number_int() {
    let schema = vld::number().int();
    assert!(schema.parse("42").is_ok());
    assert!(schema.parse("3.14").is_err());
}

#[test]
fn number_positive_negative() {
    assert!(vld::number().positive().parse("1").is_ok());
    assert!(vld::number().positive().parse("-1").is_err());
    assert!(vld::number().negative().parse("-1").is_ok());
}

#[test]
fn number_coerce() {
    let schema = vld::number().coerce();
    assert!((schema.parse(r#""42.5""#).unwrap() - 42.5).abs() < f64::EPSILON);
}

// -----------------------------------------------------------------------
// Boolean
// -----------------------------------------------------------------------

#[test]
fn boolean_basic() {
    let schema = vld::boolean();
    assert!(schema.parse("true").is_ok());
    assert!(schema.parse("false").is_ok());
    assert!(schema.parse("42").is_err());
}

#[test]
fn boolean_coerce() {
    let schema = vld::boolean().coerce();
    assert!(schema.parse(r#""true""#).unwrap());
}

// -----------------------------------------------------------------------
// Array
// -----------------------------------------------------------------------

#[test]
fn array_basic() {
    let schema = vld::array(vld::number().int());
    let result = schema.parse("[1, 2, 3]").unwrap();
    assert_eq!(result.len(), 3);
}

#[test]
fn array_min_max_len() {
    let schema = vld::array(vld::string()).min_len(1).max_len(3);
    assert!(schema.parse(r#"["a"]"#).is_ok());
    assert!(schema.parse("[]").is_err());
    assert!(schema.parse(r#"["a","b","c","d"]"#).is_err());
}

// -----------------------------------------------------------------------
// Object (dynamic)
// -----------------------------------------------------------------------

#[test]
fn object_basic() {
    let schema = vld::object()
        .field("name", vld::string().min(1))
        .field("age", vld::number().int().min(0));

    let result = schema.parse(r#"{"name":"Alice","age":30}"#).unwrap();
    assert_eq!(result.get("name").unwrap(), "Alice");
}

#[test]
fn object_strict() {
    let schema = vld::object().field("id", vld::number().int()).strict();

    assert!(schema.parse(r#"{"id":1,"extra":"oops"}"#).is_err());
}

#[test]
fn object_passthrough() {
    let schema = vld::object().field("id", vld::number().int()).passthrough();

    let result = schema.parse(r#"{"id":1,"extra":"kept"}"#).unwrap();
    assert_eq!(result.get("extra").unwrap(), "kept");
}

#[test]
fn object_field_optional_no_std() {
    let schema = vld::object()
        .field("name", vld::string())
        .field_optional("bio", vld::string());

    let result = schema.parse(r#"{"name":"Bob"}"#).unwrap();
    assert!(result.get("bio").unwrap().is_null());
}

#[test]
fn object_conditional_when() {
    let schema = vld::object()
        .field("role", vld::string())
        .field_optional("key", vld::string())
        .when("role", "admin", "key", vld::string().min(8));

    assert!(schema.parse(r#"{"role":"user"}"#).is_ok());
    assert!(schema.parse(r#"{"role":"admin","key":"short"}"#).is_err());
    assert!(schema
        .parse(r#"{"role":"admin","key":"long-enough-key"}"#)
        .is_ok());
}

// -----------------------------------------------------------------------
// Modifiers
// -----------------------------------------------------------------------

#[test]
fn optional_modifier() {
    let schema = vld::string().optional();
    assert_eq!(schema.parse_value(&serde_json::Value::Null).unwrap(), None);
    assert_eq!(schema.parse(r#""hi""#).unwrap(), Some("hi".to_string()));
}

#[test]
fn nullable_modifier() {
    let schema = vld::number().nullable();
    assert_eq!(schema.parse_value(&serde_json::Value::Null).unwrap(), None);
    assert_eq!(schema.parse("42").unwrap(), Some(42.0));
}

#[test]
fn with_default_modifier() {
    let schema = vld::string().with_default("default".into());
    assert_eq!(
        schema.parse_value(&serde_json::Value::Null).unwrap(),
        "default"
    );
}

#[test]
fn catch_modifier() {
    let schema = vld::number().int().catch(0);
    assert_eq!(schema.parse(r#""not a number""#).unwrap(), 0);
}

// -----------------------------------------------------------------------
// Combinators
// -----------------------------------------------------------------------

#[test]
fn union_combinator() {
    let schema = vld::union(vld::string(), vld::number());
    assert!(schema.parse(r#""hello""#).is_ok());
    assert!(schema.parse("42").is_ok());
    assert!(schema.parse("true").is_err());
}

#[test]
fn intersection_combinator() {
    let schema = vld::intersection(vld::number().min(0.0), vld::number().max(100.0));
    assert!(schema.parse("50").is_ok());
    assert!(schema.parse("150").is_err());
}

#[test]
fn refine_combinator() {
    let schema = vld::string().refine(|s: &String| s.starts_with("a"), "must start with a");
    assert!(schema.parse(r#""apple""#).is_ok());
    assert!(schema.parse(r#""banana""#).is_err());
}

#[test]
fn transform_combinator() {
    let schema = vld::string().transform(|s: String| s.len());
    assert_eq!(schema.parse(r#""hello""#).unwrap(), 5);
}

#[test]
fn message_combinator() {
    let schema = vld::string().min(5).message("nope");
    let err = schema.parse(r#""ab""#).unwrap_err();
    assert_eq!(err.issues[0].message, "nope");
}

// -----------------------------------------------------------------------
// Literal / Enum / Any
// -----------------------------------------------------------------------

#[test]
fn literal_schema() {
    assert!(vld::literal("admin").parse(r#""admin""#).is_ok());
    assert!(vld::literal("admin").parse(r#""user""#).is_err());
    assert!(vld::literal(42i64).parse("42").is_ok());
}

#[test]
fn enum_schema() {
    let schema = vld::enumeration(&["a", "b", "c"]);
    assert!(schema.parse(r#""a""#).is_ok());
    assert!(schema.parse(r#""d""#).is_err());
}

#[test]
fn any_schema() {
    let schema = vld::any();
    assert!(schema.parse("null").is_ok());
    assert!(schema.parse("42").is_ok());
    assert!(schema.parse(r#"[1, "two", null]"#).is_ok());
}

// -----------------------------------------------------------------------
// Discriminated union
// -----------------------------------------------------------------------

#[test]
fn discriminated_union_basic() {
    let schema = vld::discriminated_union("type")
        .variant("a", vld::object().field("type", vld::literal("a")))
        .variant("b", vld::object().field("type", vld::literal("b")));

    assert!(schema.parse(r#"{"type":"a"}"#).is_ok());
    assert!(schema.parse(r#"{"type":"c"}"#).is_err());
}

// -----------------------------------------------------------------------
// Record / Map / Set
// -----------------------------------------------------------------------

#[test]
fn record_schema() {
    let schema = vld::record(vld::number());
    let result = schema.parse(r#"{"a":1,"b":2}"#).unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn set_schema() {
    let schema = vld::set(vld::string());
    let result = schema.parse(r#"["a","b","c"]"#).unwrap();
    assert_eq!(result.len(), 3);
}

// -----------------------------------------------------------------------
// schema! macro (no serialize/openapi needed)
// -----------------------------------------------------------------------

vld::schema! {
    #[derive(Debug)]
    pub struct NoFeatureUser {
        pub name: String => vld::string().min(1).max(50),
        pub age: Option<i64> => vld::number().int().optional(),
    }
}

#[test]
fn schema_macro_parse() {
    let user = NoFeatureUser::parse(r#"{"name":"Alice"}"#).unwrap();
    assert_eq!(user.name, "Alice");
    assert_eq!(user.age, None);
}

#[test]
fn schema_macro_error() {
    let err = NoFeatureUser::parse(r#"{"name":"","age":"not"}"#).unwrap_err();
    assert!(err.issues.len() >= 2);
}

#[test]
fn schema_macro_vld_parse_trait() {
    let val = serde_json::json!({"name": "Bob", "age": 25});
    let user = NoFeatureUser::vld_parse_value(&val).unwrap();
    assert_eq!(user.name, "Bob");
    assert_eq!(user.age, Some(25));
}

// -----------------------------------------------------------------------
// Error system works without serde
// -----------------------------------------------------------------------

#[test]
fn error_display() {
    let err = vld::string().email().parse(r#""nope""#).unwrap_err();
    let display = format!("{}", err);
    assert!(!display.is_empty());
}

#[test]
fn error_issue_builder() {
    let mut errors = VldError::new();
    errors
        .issue(IssueCode::Custom {
            code: "test".into(),
        })
        .message("hello")
        .path_field("field")
        .finish();
    assert_eq!(errors.issues.len(), 1);
}

#[test]
fn error_merge() {
    let a = VldError::single(IssueCode::MissingField, "a");
    let b = VldError::single(IssueCode::MissingField, "b");
    let merged = a.merge(b);
    assert_eq!(merged.issues.len(), 2);
}

// -----------------------------------------------------------------------
// Formatting
// -----------------------------------------------------------------------

#[test]
fn format_prettify() {
    let err = vld::number().int().parse(r#""text""#).unwrap_err();
    let pretty = vld::format::prettify_error(&err);
    assert!(!pretty.is_empty());
}

#[test]
fn format_flatten() {
    let err = vld::object()
        .field("x", vld::string())
        .parse(r#"{"x": 42}"#)
        .unwrap_err();
    let flat = vld::format::flatten_error(&err);
    assert!(!flat.field_errors.is_empty() || !flat.form_errors.is_empty());
}

// -----------------------------------------------------------------------
// i18n
// -----------------------------------------------------------------------

#[test]
fn i18n_translate() {
    let resolver = vld::i18n::russian();
    let err = vld::number().min(10.0).parse("5").unwrap_err();
    let translated = vld::i18n::translate_error(&err, &resolver);
    assert!(!translated.issues[0].message.is_empty());
}

// -----------------------------------------------------------------------
// Diff
// -----------------------------------------------------------------------

#[cfg(feature = "diff")]
#[test]
fn diff_basic() {
    let old = serde_json::json!({"type":"string"});
    let new = serde_json::json!({"type":"number"});
    let diff = vld::diff::diff_schemas(&old, &new);
    assert!(diff.has_breaking());
}

// -----------------------------------------------------------------------
// Lazy / Custom / Preprocess
// -----------------------------------------------------------------------

#[test]
fn lazy_schema() {
    let schema = vld::lazy(|| vld::string().min(1));
    assert!(schema.parse(r#""hi""#).is_ok());
}

#[test]
fn custom_schema() {
    let schema = vld::custom(|v: &serde_json::Value| {
        v.as_str()
            .filter(|s| s.len() > 2)
            .map(|s| s.to_string())
            .ok_or_else(|| "Need string > 2 chars".to_string())
    });
    assert!(schema.parse(r#""abc""#).is_ok());
    assert!(schema.parse(r#""ab""#).is_err());
}

#[test]
fn preprocess_schema() {
    let schema = vld::preprocess(
        |v| match v.as_str() {
            Some(s) => serde_json::json!(s.trim()),
            None => v.clone(),
        },
        vld::string().min(1),
    );
    assert!(schema.parse(r#""  hello  ""#).is_ok());
}
