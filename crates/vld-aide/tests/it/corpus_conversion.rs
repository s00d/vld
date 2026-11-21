//! Corpus: `vld_to_schemars` keeps every JSON Schema (vld's own and hand
//! written) unchanged, and turns a non-schema into the `false` schema.

use serde_json::{Value, json};
use vld::openapi::JsonSchema;
use vld_aide::{schemars, vld_to_schemars};

/// Given JSON Schemas of every shape, when aide users convert them, then the
/// schemars schema serializes back to the same document.
#[test]
fn schemas_convert_unchanged() {
    let schemas: Vec<Value> = vec![
        json!({"type": "string"}),
        json!({"type": "string", "minLength": 2, "maxLength": 50, "format": "email"}),
        json!({"type": "integer", "minimum": 0, "maximum": 100}),
        json!({"type": "boolean"}),
        json!({"type": "object", "required": ["name", "email"], "properties": {
            "name": {"type": "string", "minLength": 1},
            "email": {"type": "string", "format": "email"}
        }}),
        json!({"type": "array", "items": {"type": "string"}, "minItems": 1, "maxItems": 10}),
        json!({"oneOf": [{"type": "string"}, {"type": "integer"}]}),
        json!(true),
        json!(false),
        vld::string().min(3).max(100).email().json_schema(),
        vld::number().min(0.0).max(100.0).json_schema(),
    ];
    for js in schemas {
        let s = vld_to_schemars(&js);
        assert_eq!(serde_json::to_value(&s).unwrap(), js);
    }
    for bad in [json!("not a schema"), json!(1), json!(null)] {
        assert_eq!(
            vld_to_schemars(&bad),
            schemars::Schema::from(false),
            "{bad}"
        );
    }
}
