//! Example: generate TypeScript Zod schemas from JSON Schema definitions.
//!
//! Run:
//! ```sh
//! cargo run -p vld-ts --example generate_zod
//! ```

use vld_ts::{generate_zod_file, json_schema_to_zod};

fn main() {
    // --- Single schema ---
    println!("=== Single schema ===\n");

    let user_schema = serde_json::json!({
        "type": "object",
        "required": ["name", "email"],
        "properties": {
            "name": {
                "type": "string",
                "minLength": 2,
                "maxLength": 50,
                "description": "User's full name"
            },
            "email": {
                "type": "string",
                "format": "email"
            },
            "age": {
                "type": "integer",
                "minimum": 0,
                "maximum": 150
            },
            "tags": {
                "type": "array",
                "items": { "type": "string", "minLength": 1 },
                "maxItems": 10
            }
        }
    });

    let zod = json_schema_to_zod(&user_schema);
    println!("const UserSchema = {};\n", zod);

    // --- Multiple schemas in a file ---
    println!("=== Generated file ===\n");

    let schemas = vec![
        (
            "User",
            serde_json::json!({
                "type": "object",
                "required": ["name", "email"],
                "properties": {
                    "name": {"type": "string", "minLength": 2},
                    "email": {"type": "string", "format": "email"}
                }
            }),
        ),
        (
            "Product",
            serde_json::json!({
                "type": "object",
                "required": ["title", "price"],
                "properties": {
                    "title": {"type": "string", "minLength": 1},
                    "price": {"type": "number", "minimum": 0},
                    "category": {
                        "type": "string",
                        "enum": ["electronics", "books", "clothing"]
                    }
                }
            }),
        ),
        (
            "ApiResponse",
            serde_json::json!({
                "type": "object",
                "required": ["status", "data"],
                "properties": {
                    "status": {"type": "string", "enum": ["ok", "error"]},
                    "data": {"type": "object"},
                    "message": {"type": "string"}
                }
            }),
        ),
    ];

    let file_content = generate_zod_file(&schemas);
    println!("{}", file_content);

    // --- Nullable / union types ---
    println!("=== Nullable & union ===\n");

    let nullable = serde_json::json!({
        "oneOf": [
            {"type": "string"},
            {"type": "null"}
        ]
    });
    println!("nullable string: {}", json_schema_to_zod(&nullable));

    let union = serde_json::json!({
        "oneOf": [
            {"type": "string"},
            {"type": "number"},
            {"type": "boolean"}
        ]
    });
    println!("union:           {}", json_schema_to_zod(&union));

    // --- Nested objects ---
    println!("\n=== Nested objects ===\n");

    let nested = serde_json::json!({
        "type": "object",
        "required": ["address"],
        "properties": {
            "address": {
                "type": "object",
                "required": ["street", "city"],
                "properties": {
                    "street": {"type": "string"},
                    "city": {"type": "string"},
                    "zip": {"type": "string", "pattern": "^\\d{5}$"}
                }
            }
        }
    });
    println!("{}", json_schema_to_zod(&nested));
}
