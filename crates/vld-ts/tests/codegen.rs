use vld_ts::{generate_zod_file, json_schema_to_zod};

#[test]
fn string_schema() {
    let schema =
        serde_json::json!({"type": "string", "minLength": 2, "maxLength": 50, "format": "email"});
    assert_eq!(
        json_schema_to_zod(&schema),
        "z.string().min(2).max(50).email()"
    );
}

#[test]
fn integer_schema() {
    let schema = serde_json::json!({"type": "integer", "minimum": 0, "maximum": 100});
    assert_eq!(
        json_schema_to_zod(&schema),
        "z.number().int().min(0).max(100)"
    );
}

#[test]
fn boolean_schema() {
    assert_eq!(
        json_schema_to_zod(&serde_json::json!({"type": "boolean"})),
        "z.boolean()"
    );
}

#[test]
fn null_schema() {
    assert_eq!(
        json_schema_to_zod(&serde_json::json!({"type": "null"})),
        "z.null()"
    );
}

#[test]
fn array_schema() {
    let schema = serde_json::json!({"type": "array", "items": {"type": "string"}, "minItems": 1});
    assert_eq!(json_schema_to_zod(&schema), "z.array(z.string()).min(1)");
}

#[test]
fn array_max_items() {
    let schema = serde_json::json!({"type": "array", "items": {"type": "number"}, "maxItems": 10});
    let result = json_schema_to_zod(&schema);
    assert!(result.contains("z.array(z.number())"));
    assert!(result.contains(".max(10)"));
}

#[test]
fn array_unique_items() {
    let schema =
        serde_json::json!({"type": "array", "items": {"type": "string"}, "uniqueItems": true});
    assert!(json_schema_to_zod(&schema).contains("uniqueItems"));
}

#[test]
fn object_schema() {
    let schema = serde_json::json!({
        "type": "object",
        "required": ["name"],
        "properties": {
            "name": {"type": "string"},
            "age": {"type": "integer"}
        }
    });
    let result = json_schema_to_zod(&schema);
    assert!(result.contains("z.object("));
    assert!(result.contains("name: z.string()"));
    assert!(result.contains("age: z.number().int().optional()"));
}

#[test]
fn object_strict() {
    let schema = serde_json::json!({
        "type": "object",
        "properties": {"a": {"type": "string"}},
        "required": ["a"],
        "additionalProperties": false
    });
    assert!(json_schema_to_zod(&schema).contains(".strict()"));
}

#[test]
fn object_record() {
    let schema = serde_json::json!({
        "type": "object",
        "additionalProperties": {"type": "number"}
    });
    assert!(json_schema_to_zod(&schema).contains("z.record(z.string(), z.number())"));
}

#[test]
fn nullable() {
    let schema = serde_json::json!({"oneOf": [{"type": "string"}, {"type": "null"}]});
    assert_eq!(json_schema_to_zod(&schema), "z.string().nullable()");
}

#[test]
fn nullable_reverse() {
    let schema = serde_json::json!({"oneOf": [{"type": "null"}, {"type": "number"}]});
    assert_eq!(json_schema_to_zod(&schema), "z.number().nullable()");
}

#[test]
fn union() {
    let schema = serde_json::json!({"oneOf": [{"type": "string"}, {"type": "number"}]});
    assert_eq!(
        json_schema_to_zod(&schema),
        "z.union([z.string(), z.number()])"
    );
}

#[test]
fn any_of() {
    let schema = serde_json::json!({"anyOf": [{"type": "string"}, {"type": "boolean"}]});
    assert_eq!(
        json_schema_to_zod(&schema),
        "z.union([z.string(), z.boolean()])"
    );
}

#[test]
fn all_of() {
    let schema = serde_json::json!({
        "allOf": [
            {"type": "object", "properties": {"a": {"type": "string"}}, "required": ["a"]},
            {"type": "object", "properties": {"b": {"type": "number"}}, "required": ["b"]}
        ]
    });
    assert!(json_schema_to_zod(&schema).contains("z.intersection("));
}

#[test]
fn all_of_single() {
    let schema = serde_json::json!({"allOf": [{"type": "string"}]});
    assert_eq!(json_schema_to_zod(&schema), "z.string()");
}

#[test]
fn enum_schema() {
    let schema = serde_json::json!({"type": "string", "enum": ["admin", "user", "mod"]});
    let result = json_schema_to_zod(&schema);
    assert!(result.contains("z.union("));
    assert!(result.contains("z.literal(\"admin\")"));
}

#[test]
fn single_literal_enum() {
    assert_eq!(
        json_schema_to_zod(&serde_json::json!({"enum": ["only"]})),
        "z.literal(\"only\")"
    );
}

#[test]
fn numeric_enum() {
    let result = json_schema_to_zod(&serde_json::json!({"enum": [1, 2, 3]}));
    assert!(result.contains("z.literal(1)"));
    assert!(result.contains("z.literal(2)"));
    assert!(result.contains("z.literal(3)"));
}

#[test]
fn boolean_enum() {
    let result = json_schema_to_zod(&serde_json::json!({"enum": [true, false]}));
    assert!(result.contains("z.literal(true)"));
    assert!(result.contains("z.literal(false)"));
}

#[test]
fn number_schema() {
    let schema = serde_json::json!({"type": "number", "minimum": 0.5, "maximum": 99.9});
    let result = json_schema_to_zod(&schema);
    assert!(result.starts_with("z.number()"));
    assert!(result.contains(".min("));
    assert!(result.contains(".max("));
}

#[test]
fn exclusive_min_max() {
    let schema =
        serde_json::json!({"type": "number", "exclusiveMinimum": 0, "exclusiveMaximum": 100});
    let result = json_schema_to_zod(&schema);
    assert!(result.contains(".gt(0)"));
    assert!(result.contains(".lt(100)"));
}

#[test]
fn multiple_of() {
    let schema = serde_json::json!({"type": "integer", "multipleOf": 5});
    assert!(json_schema_to_zod(&schema).contains(".multipleOf(5)"));
}

#[test]
fn string_formats() {
    let cases = vec![
        ("email", ".email()"),
        ("uri", ".url()"),
        ("url", ".url()"),
        ("uuid", ".uuid()"),
        ("ipv4", ".ip({ version: \"v4\" })"),
        ("ipv6", ".ip({ version: \"v6\" })"),
        ("date", ".date()"),
        ("date-time", ".datetime()"),
        ("time", ".time()"),
    ];
    for (fmt, expected) in cases {
        let schema = serde_json::json!({"type": "string", "format": fmt});
        let result = json_schema_to_zod(&schema);
        assert!(
            result.contains(expected),
            "format {} should produce {}, got {}",
            fmt,
            expected,
            result
        );
    }
}

#[test]
fn string_pattern() {
    let schema = serde_json::json!({"type": "string", "pattern": "^\\d+$"});
    assert!(json_schema_to_zod(&schema).contains(".regex(/^\\d+$/)"));
}

#[test]
fn described_schema() {
    let schema = serde_json::json!({"type": "string", "description": "User name"});
    assert_eq!(
        json_schema_to_zod(&schema),
        "z.string().describe(\"User name\")"
    );
}

#[test]
fn description_with_quotes() {
    let schema = serde_json::json!({"type": "string", "description": "A \"quoted\" field"});
    assert!(json_schema_to_zod(&schema).contains(r#".describe("A \"quoted\" field")"#));
}

#[test]
fn empty_schema() {
    assert_eq!(json_schema_to_zod(&serde_json::json!({})), "z.unknown()");
}

#[test]
fn nested_object() {
    let schema = serde_json::json!({
        "type": "object",
        "required": ["inner"],
        "properties": {
            "inner": {
                "type": "object",
                "required": ["x"],
                "properties": {
                    "x": {"type": "number"}
                }
            }
        }
    });
    let result = json_schema_to_zod(&schema);
    assert!(result.matches("z.object(").count() >= 2);
}

#[test]
fn generate_file() {
    let schemas = vec![(
        "User",
        serde_json::json!({
            "type": "object",
            "required": ["name"],
            "properties": {"name": {"type": "string"}}
        }),
    )];
    let ts = generate_zod_file(&schemas);
    assert!(ts.contains("import { z } from \"zod\""));
    assert!(ts.contains("export const UserSchema"));
    assert!(ts.contains("export type User"));
}

#[test]
fn generate_file_multiple() {
    let schemas = vec![
        ("Foo", serde_json::json!({"type": "string"})),
        ("Bar", serde_json::json!({"type": "number"})),
        ("Baz", serde_json::json!({"type": "boolean"})),
    ];
    let ts = generate_zod_file(&schemas);
    assert!(ts.contains("export const FooSchema = z.string()"));
    assert!(ts.contains("export type Foo = z.infer<typeof FooSchema>"));
    assert!(ts.contains("export const BarSchema = z.number()"));
    assert!(ts.contains("export const BazSchema = z.boolean()"));
    assert!(ts.contains("Auto-generated"));
}
