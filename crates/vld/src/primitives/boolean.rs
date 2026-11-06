use serde_json::Value;

use crate::error::VldError;
use crate::schema::VldSchema;

/// Schema for boolean validation. Created via [`vld::boolean()`](crate::boolean).
#[derive(Clone)]
pub struct ZBoolean {
    coerce: bool,
    custom_type_error: Option<String>,
}

impl ZBoolean {
    /// Create the schema (the same as its `vld::` constructor function).
    pub fn new() -> Self {
        Self {
            coerce: false,
            custom_type_error: None,
        }
    }

    /// Set a custom error message for type mismatch (and failed coercion).
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    /// Coerce truthy/falsy values to booleans.
    ///
    /// Accepted coercions:
    /// - Strings: `"true"`, `"1"` → `true`; `"false"`, `"0"` → `false`
    /// - Numbers: `0` → `false`; anything else → `true`
    pub fn coerce(mut self) -> Self {
        self.coerce = true;
        self
    }
}

impl Default for ZBoolean {
    fn default() -> Self {
        Self::new()
    }
}

impl VldSchema for ZBoolean {
    type Output = bool;

    fn parse_value(&self, value: &Value) -> Result<bool, VldError> {
        match value {
            Value::Bool(b) => Ok(*b),
            Value::String(s) if self.coerce => match s.as_str() {
                "true" | "1" => Ok(true),
                "false" | "0" => Ok(false),
                _ => Err(crate::primitives::type_error(
                    &self.custom_type_error,
                    "boolean",
                    value,
                )),
            },
            Value::Number(n) if self.coerce => Ok(n.as_f64() != Some(0.0)),
            _ => Err(crate::primitives::type_error(
                &self.custom_type_error,
                "boolean",
                value,
            )),
        }
    }
}

/// Generate a JSON Schema representation.
#[cfg(feature = "openapi")]
impl crate::openapi::JsonSchema for ZBoolean {
    fn json_schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "boolean"})
    }
}

#[cfg(feature = "openapi")]
impl crate::openapi::CollectNestedSchemas for ZBoolean {}
