use serde_json::Value;

use crate::error::{value_type_name, IssueCode, VldError};
use crate::schema::VldSchema;

/// Schema for boolean validation. Created via [`vld::boolean()`](crate::boolean).
#[derive(Clone)]
pub struct ZBoolean {
    coerce: bool,
}

impl ZBoolean {
    pub fn new() -> Self {
        Self { coerce: false }
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
        if let Some(b) = value.as_bool() {
            return Ok(b);
        }

        if self.coerce {
            match value {
                Value::String(s) => match s.as_str() {
                    "true" | "1" => Ok(true),
                    "false" | "0" => Ok(false),
                    _ => Err(VldError::single_with_value(
                        IssueCode::InvalidType {
                            expected: "boolean".into(),
                            received: "string".into(),
                        },
                        format!("Cannot coerce \"{}\" to boolean", s),
                        value,
                    )),
                },
                Value::Number(n) => Ok(n.as_f64() != Some(0.0)),
                _ => Err(VldError::single_with_value(
                    IssueCode::InvalidType {
                        expected: "boolean".into(),
                        received: value_type_name(value),
                    },
                    format!("Expected boolean, received {}", value_type_name(value)),
                    value,
                )),
            }
        } else {
            Err(VldError::single_with_value(
                IssueCode::InvalidType {
                    expected: "boolean".into(),
                    received: value_type_name(value),
                },
                format!("Expected boolean, received {}", value_type_name(value)),
                value,
            ))
        }
    }
}
