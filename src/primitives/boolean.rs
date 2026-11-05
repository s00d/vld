use serde_json::Value;

use crate::error::{IssueCode, VldError};
use crate::schema::VldSchema;

/// Schema for boolean validation. Created via [`vld::boolean()`](crate::boolean).
#[derive(Clone)]
pub struct ZBoolean {
    coerce: bool,
    custom_type_error: Option<String>,
}

impl ZBoolean {
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

impl ZBoolean {
    /// Generate a JSON Schema representation.
    ///
    /// Requires the `openapi` feature.
    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "boolean"})
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
                _ => Err(VldError::single_with_value(
                    IssueCode::InvalidType {
                        expected: "boolean".into(),
                        received: "string".into(),
                    },
                    self.custom_type_error
                        .clone()
                        .unwrap_or_else(|| format!("Cannot coerce \"{}\" to boolean", s)),
                    value,
                )),
            },
            Value::Number(n) if self.coerce => Ok(n.as_f64() != Some(0.0)),
            _ => Err(super::type_error(
                &self.custom_type_error,
                "boolean",
                "boolean",
                value,
            )),
        }
    }
}
