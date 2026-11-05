//! Regression tests for the live end-to-end findings (0.5.0).

use serde_json::{json, Value};
use vld::json_schema::JsonSchema;
use vld_utoipa::json_schema_to_schema;

fn conv(v: &Value) -> Value {
    serde_json::to_value(json_schema_to_schema(v)).unwrap()
}

#[test]
fn u64_bounds_stay_integers() {
    let out = conv(&vld::u64().json_schema());
    assert_eq!(out["maximum"], json!(u64::MAX));
    assert!(out["maximum"].is_u64(), "{out}");
}

#[test]
fn tuple_keeps_prefix_items() {
    let schema = (vld::string(), vld::u16()).json_schema();
    let out = conv(&schema);
    assert_eq!(out["prefixItems"][0]["type"], "string");
    assert_eq!(out["prefixItems"][1]["type"], "integer");
    assert_eq!(out["items"], json!(false));
    assert_eq!(out["minItems"], 2);
    assert_eq!(out["maxItems"], 2);
}
