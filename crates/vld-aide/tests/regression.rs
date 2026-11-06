//! Regression tests for the live end-to-end findings (0.5.0): OpenAPI 3.0
//! generator settings get OpenAPI 3.0 schemas.

use serde_json::Value;
use vld_aide::impl_json_schema;
use vld_aide::schemars;

vld::schema! {
    #[derive(Debug)]
    pub struct Oas3 {
        pub pair: (String, u16) => (vld::string(), vld::u16()),
        pub nick: Option<String> => vld::string().nullable(),
    }
}

impl_json_schema!(Oas3);

/// Every key in `v`, recursively.
fn keys(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, v) in m {
                out.push(k.clone());
                keys(v, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|v| keys(v, out)),
        _ => {}
    }
}

#[test]
fn openapi3_settings_yield_openapi3_schemas() {
    let gen = schemars::generate::SchemaSettings::openapi3().into_generator();
    let root = gen.into_root_schema_for::<Oas3>();
    let json = serde_json::to_value(&root).unwrap();
    let mut all = Vec::new();
    keys(&json, &mut all);
    for bad in ["prefixItems", "additionalItems"] {
        assert!(!all.iter().any(|k| k == bad), "{bad} in {json}");
    }

    let pair = &json["properties"]["pair"];
    assert_eq!(pair["type"], "array");
    assert!(pair["items"].is_object(), "{pair}");
    assert_eq!(pair["minItems"], 2);
    assert_eq!(pair["maxItems"], 2);

    let nick = &json["properties"]["nick"];
    assert_eq!(nick["type"], "string", "{nick}");
    assert_eq!(nick["nullable"], true, "{nick}");
    assert!(nick.get("enum").is_none(), "{nick}");
}
