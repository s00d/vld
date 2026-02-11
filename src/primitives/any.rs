use serde_json::Value;

use crate::error::VldError;
use crate::schema::VldSchema;

/// Schema that accepts any JSON value. Created via [`vld::any()`](crate::any).
#[derive(Clone, Copy)]
pub struct ZAny;

impl ZAny {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ZAny {
    fn default() -> Self {
        Self::new()
    }
}

impl ZAny {
    /// Generate a JSON Schema representation (empty schema = any).
    ///
    /// Requires the `openapi` feature.
    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        serde_json::json!({})
    }
}

impl VldSchema for ZAny {
    type Output = Value;

    fn parse_value(&self, value: &Value) -> Result<Value, VldError> {
        Ok(value.clone())
    }
}
