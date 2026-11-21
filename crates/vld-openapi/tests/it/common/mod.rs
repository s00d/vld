//! Helpers shared by every module of the `it` test binary.
#![allow(dead_code)] // each feature combination uses a different subset

use serde_json::Value;
use vld::error::VldError;
use vld::schema::{DynSchema, VldSchema};

/// `(path, code key)` of every issue, in order (`""` is the root path).
pub fn pairs(err: &VldError) -> Vec<(String, String)> {
    err.issues
        .iter()
        .map(|i| {
            (
                i.path.iter().map(ToString::to_string).collect(),
                i.code.key().to_string(),
            )
        })
        .collect()
}

/// `validate_with_schema` as `Ok` or the `(path, code)` issues.
pub fn verdict(schema: &Value, value: &Value) -> Result<(), Vec<(String, String)>> {
    vld_openapi::validator::validate_with_schema(schema, value).map_err(|e| pairs(&e))
}

/// `value` passes `schema` (`validate_with_schema`).
#[track_caller]
pub fn valid(schema: &Value, value: &Value) {
    if let Err(e) = verdict(schema, value) {
        panic!("{value} rejected by {schema}: {e:?}");
    }
}

/// `value` fails `schema` with exactly these `(path, code)` issues.
#[track_caller]
pub fn invalid(schema: &Value, value: &Value, expected: &[(&str, &str)]) {
    match verdict(schema, value) {
        Ok(()) => panic!("{value} accepted by {schema}"),
        Err(got) => {
            let got: Vec<(&str, &str)> =
                got.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
            assert_eq!(got, expected, "{value} against {schema}");
        }
    }
}

/// A type-erased schema whose output is JSON, so random schema trees can be
/// composed (`array(Erased)`, `Erased.optional()`, …).
pub struct Erased(pub Box<dyn DynSchema + Send + Sync>);

impl Erased {
    pub fn new<S: DynSchema + Send + Sync + 'static>(s: S) -> Self {
        Self(Box::new(s))
    }
}

impl VldSchema for Erased {
    type Output = Value;
    fn parse_value(&self, value: &Value) -> Result<Value, VldError> {
        self.0.dyn_parse(value)
    }
    fn parse_missing(&self) -> Result<Value, VldError> {
        self.0.dyn_parse_missing()
    }
}

impl vld::openapi::JsonSchema for Erased {
    fn json_schema(&self) -> Value {
        self.0.dyn_json_schema()
    }
    fn is_required(&self) -> bool {
        self.0.dyn_is_required()
    }
}

impl vld::openapi::CollectNestedSchemas for Erased {
    fn collect_nested_schemas(&self, out: &mut Vec<vld::schema::NestedSchemaEntry>) {
        self.0.dyn_collect_nested_schemas(out)
    }
}
