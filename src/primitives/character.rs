use serde_json::Value;

use crate::error::{IssueCode, SizeOrigin, VldError};
use crate::schema::VldSchema;

/// Schema for a single `char`: a JSON string of exactly one Unicode scalar
/// value. Created via [`vld::char()`](crate::char).
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// assert_eq!(vld::char().parse(r#""é""#).unwrap(), 'é');
/// assert!(vld::char().parse(r#""ab""#).is_err());
/// assert!(vld::char().parse(r#""""#).is_err());
/// ```
#[derive(Clone, Default)]
pub struct ZChar {
    custom_type_error: Option<String>,
}

impl ZChar {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a custom error message for type mismatch (non-string input).
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }
}

impl VldSchema for ZChar {
    type Output = char;

    fn parse_value(&self, value: &Value) -> Result<char, VldError> {
        let s = value.as_str().ok_or_else(|| {
            crate::primitives::type_error(&self.custom_type_error, "string", value)
        })?;
        let mut chars = s.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => Ok(c),
            _ => Err(VldError::from_code_with_value(
                IssueCode::InvalidLength {
                    origin: SizeOrigin::String,
                    expected: 1,
                },
                value,
            )),
        }
    }
}

/// `{"type": "string", "minLength": 1, "maxLength": 1}`.
#[cfg(feature = "openapi")]
impl crate::openapi::JsonSchema for ZChar {
    fn json_schema(&self) -> Value {
        serde_json::json!({"type": "string", "minLength": 1, "maxLength": 1})
    }
}

#[cfg(feature = "openapi")]
impl crate::openapi::CollectNestedSchemas for ZChar {}
