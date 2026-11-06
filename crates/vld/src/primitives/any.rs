use serde_json::Value;

use crate::error::{IssueCode, VldError};
use crate::schema::VldSchema;

/// Schema that accepts any JSON value. Created via [`vld::any()`](crate::any).
#[derive(Clone, Copy)]
pub struct ZAny;

impl ZAny {
    /// Create the schema (the same as its `vld::` constructor function).
    pub fn new() -> Self {
        Self
    }
}

impl Default for ZAny {
    fn default() -> Self {
        Self::new()
    }
}

impl VldSchema for ZAny {
    type Output = Value;

    fn parse_value(&self, value: &Value) -> Result<Value, VldError> {
        Ok(value.clone())
    }
}

/// Generate a JSON Schema representation (empty schema = any).
#[cfg(feature = "openapi")]
impl crate::openapi::JsonSchema for ZAny {
    fn json_schema(&self) -> serde_json::Value {
        serde_json::json!({})
    }
    /// Accepts `null`, so a missing key passes.
    fn is_required(&self) -> bool {
        false
    }
}

#[cfg(feature = "openapi")]
impl crate::openapi::CollectNestedSchemas for ZAny {}

/// Schema that accepts only `null`. Created via [`vld::null()`](crate::null).
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// assert_eq!(vld::null().parse("null").unwrap(), ());
/// assert!(vld::null().parse("0").is_err());
/// ```
#[derive(Clone, Default)]
pub struct ZNull {
    custom_type_error: Option<String>,
}

impl ZNull {
    /// Create the schema (the same as its `vld::` constructor function).
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a custom error message for non-null input.
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }
}

impl VldSchema for ZNull {
    type Output = ();

    fn parse_value(&self, value: &Value) -> Result<(), VldError> {
        if value.is_null() {
            Ok(())
        } else {
            Err(crate::primitives::type_error(
                &self.custom_type_error,
                "null",
                value,
            ))
        }
    }
}

/// `{"type": "null"}`.
#[cfg(feature = "openapi")]
impl crate::openapi::JsonSchema for ZNull {
    fn json_schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "null"})
    }
    /// A missing key is `null`, which passes.
    fn is_required(&self) -> bool {
        false
    }
}

#[cfg(feature = "openapi")]
impl crate::openapi::CollectNestedSchemas for ZNull {}

/// Schema that rejects every value (`invalid_type`, expected `never`).
/// Created via [`vld::never()`](crate::never).
///
/// Useful in unions / discriminated unions and for fields that must not be
/// sent; its output type [`Infallible`](std::convert::Infallible) cannot exist.
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// let err = vld::never().parse("1").unwrap_err();
/// assert_eq!(err.issues[0].code.key(), "invalid_type");
/// assert!(vld::never().optional().parse("null").unwrap().is_none());
/// ```
#[derive(Clone, Copy, Default)]
pub struct ZNever;

impl ZNever {
    /// Create the schema (the same as its `vld::` constructor function).
    pub fn new() -> Self {
        Self
    }
}

impl VldSchema for ZNever {
    type Output = std::convert::Infallible;

    fn parse_value(&self, value: &Value) -> Result<std::convert::Infallible, VldError> {
        Err(VldError::from_code_with_value(
            IssueCode::InvalidType {
                expected: "never".to_string(),
                received: crate::error::value_type_name(value),
            },
            value,
        ))
    }
}

/// `{"not": {}}` (matches nothing).
#[cfg(feature = "openapi")]
impl crate::openapi::JsonSchema for ZNever {
    fn json_schema(&self) -> serde_json::Value {
        serde_json::json!({"not": {}})
    }
}

#[cfg(feature = "openapi")]
impl crate::openapi::CollectNestedSchemas for ZNever {}
