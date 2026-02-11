use serde_json::Value;

use crate::error::{value_type_name, IssueCode, VldError};
use crate::schema::VldSchema;

/// Schema for string enum validation. Created via [`vld::enumeration()`](crate::enumeration).
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// let role = vld::enumeration(&["admin", "user", "moderator"]);
/// assert!(role.parse(r#""admin""#).is_ok());
/// assert!(role.parse(r#""hacker""#).is_err());
/// ```
#[derive(Clone)]
pub struct ZEnum {
    variants: Vec<String>,
}

impl ZEnum {
    pub fn new(variants: &[&str]) -> Self {
        Self {
            variants: variants.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Create from a Vec of Strings.
    pub fn from_strings(variants: Vec<String>) -> Self {
        Self { variants }
    }
}

impl ZEnum {
    /// Generate a JSON Schema representation.
    ///
    /// Requires the `openapi` feature.
    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "string", "enum": self.variants})
    }
}

impl VldSchema for ZEnum {
    type Output = String;

    fn parse_value(&self, value: &Value) -> Result<String, VldError> {
        let s = value.as_str().ok_or_else(|| {
            VldError::single_with_value(
                IssueCode::InvalidType {
                    expected: "string".to_string(),
                    received: value_type_name(value),
                },
                format!("Expected string, received {}", value_type_name(value)),
                value,
            )
        })?;

        if self.variants.iter().any(|v| v == s) {
            Ok(s.to_string())
        } else {
            Err(VldError::single_with_value(
                IssueCode::Custom {
                    code: "invalid_enum_value".to_string(),
                },
                format!(
                    "Invalid enum value: \"{}\". Expected one of: {}",
                    s,
                    self.variants
                        .iter()
                        .map(|v| format!("\"{}\"", v))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                value,
            ))
        }
    }
}
