use serde_json::Value;

use crate::error::VldError;
use crate::schema::VldSchema;

/// Wraps a schema with a human-readable description.
///
/// Created via [`VldSchema::describe()`].
///
/// The description is metadata-only and does not affect validation.
pub struct ZDescribe<T: VldSchema> {
    inner: T,
    description: String,
}

impl<T: VldSchema> ZDescribe<T> {
    pub fn new(inner: T, description: &str) -> Self {
        Self {
            inner,
            description: description.to_string(),
        }
    }

    /// Get the description string.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Access the inner schema.
    pub fn inner_schema(&self) -> &T {
        &self.inner
    }
}

impl<T: VldSchema> VldSchema for ZDescribe<T> {
    type Output = T::Output;

    fn parse_value(&self, value: &Value) -> Result<T::Output, VldError> {
        self.inner.parse_value(value)
    }
}
