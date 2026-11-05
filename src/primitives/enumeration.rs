use serde_json::Value;

use crate::error::{IssueCode, VldError};
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
    custom_type_error: Option<String>,
}

impl ZEnum {
    pub fn new(variants: &[&str]) -> Self {
        Self {
            variants: variants.iter().map(|s| s.to_string()).collect(),
            custom_type_error: None,
        }
    }

    /// Create from a Vec of Strings.
    pub fn from_strings(variants: Vec<String>) -> Self {
        Self {
            variants,
            custom_type_error: None,
        }
    }

    /// Set a custom error message for type mismatch (non-string input).
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }
}

impl VldSchema for ZEnum {
    type Output = String;

    fn parse_value(&self, value: &Value) -> Result<String, VldError> {
        let s = value.as_str().ok_or_else(|| {
            crate::primitives::type_error(&self.custom_type_error, "string", value)
        })?;

        if self.variants.iter().any(|v| v == s) {
            Ok(s.to_string())
        } else {
            Err(VldError::from_code_with_value(
                IssueCode::InvalidEnumValue {
                    options: self.variants.clone(),
                },
                value,
            ))
        }
    }
}

/// Generate a JSON Schema representation.
#[cfg(feature = "openapi")]
impl crate::openapi::JsonSchema for ZEnum {
    fn json_schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "string", "enum": self.variants})
    }
}

#[cfg(feature = "openapi")]
impl crate::openapi::CollectNestedSchemas for ZEnum {}
