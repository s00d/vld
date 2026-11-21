//! Helpers shared by every module of the `it` test binary.
#![allow(dead_code)] // each feature combination uses a different subset

use serde_json::Value;
use vld::error::{ValidationIssue, VldError};
use vld::schema::{DynSchema, VldSchema};

/// `.a.b[0]`: the issue path as one string (`""` for the root).
pub fn path_of(issue: &ValidationIssue) -> String {
    issue.path.iter().map(ToString::to_string).collect()
}

/// `(path, code key)` of every issue, in order: what the tests compare
/// (messages are checked only where a story is about messages).
pub fn issues(err: &VldError) -> Vec<(String, String)> {
    err.issues
        .iter()
        .map(|i| (path_of(i), i.code.key().to_string()))
        .collect()
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

#[cfg(feature = "openapi")]
impl vld::openapi::JsonSchema for Erased {
    fn json_schema(&self) -> Value {
        self.0.dyn_json_schema()
    }
    fn is_required(&self) -> bool {
        self.0.dyn_is_required()
    }
}

#[cfg(feature = "openapi")]
impl vld::openapi::CollectNestedSchemas for Erased {
    fn collect_nested_schemas(&self, out: &mut Vec<vld::schema::NestedSchemaEntry>) {
        self.0.dyn_collect_nested_schemas(out)
    }
}

/// A fresh temporary directory, removed on drop.
pub fn tempdir() -> tempfile::TempDir {
    tempfile::tempdir().expect("create temp dir")
}
