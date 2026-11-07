use serde_json::Value;

use crate::error::VldError;
use crate::schema::VldSchema;

/// Override the error message for an inner schema.
///
/// Created via [`VldSchema::message()`]. On validation failure the message of
/// **every** issue is replaced (codes, paths and received values are kept).
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// let schema = vld::string().min(3).message("Too short");
/// let err = schema.parse(r#""ab""#).unwrap_err();
/// assert_eq!(err.issues[0].message, "Too short");
/// ```
pub struct ZMessage<T: VldSchema> {
    inner: T,
    msg: String,
}

impl<T: VldSchema> ZMessage<T> {
    /// Every issue of `err` gets this schema's message.
    fn relabel(&self, mut err: VldError) -> VldError {
        for issue in &mut err.issues {
            issue.message = self.msg.clone();
        }
        err
    }

    /// Create the schema (the same as its `vld::` constructor function).
    pub fn new(inner: T, msg: impl Into<String>) -> Self {
        Self {
            inner,
            msg: msg.into(),
        }
    }

    /// Access the inner schema (for nested OpenAPI component collection).
    pub fn inner_schema(&self) -> &T {
        &self.inner
    }
}

impl<T: VldSchema> VldSchema for ZMessage<T> {
    type Output = T::Output;

    fn parse_value(&self, value: &Value) -> Result<T::Output, VldError> {
        self.inner.parse_value(value).map_err(|e| self.relabel(e))
    }
    fn parse_missing(&self) -> Result<T::Output, VldError> {
        self.inner.parse_missing().map_err(|e| self.relabel(e))
    }
    fn output_json(&self, o: &Self::Output) -> Option<Value> {
        self.inner.output_json(o)
    }
}
