use serde_json::Value;

use crate::error::VldError;
use crate::schema::VldSchema;

/// Wraps a schema to provide a default value when the input is `null` or missing.
///
/// - If the value is `null` → returns the default value.
/// - If the value is present and valid → returns the parsed value.
/// - If the value is present but invalid → returns an error (NOT the default).
pub struct ZDefault<T: VldSchema>
where
    T::Output: Clone,
{
    inner: T,
    default_value: T::Output,
}

impl<T: VldSchema> ZDefault<T>
where
    T::Output: Clone,
{
    pub fn new(inner: T, default_value: T::Output) -> Self {
        Self {
            inner,
            default_value,
        }
    }

    /// Access the inner schema (for JSON Schema generation).
    pub fn inner_schema(&self) -> &T {
        &self.inner
    }
}

impl<T: VldSchema> VldSchema for ZDefault<T>
where
    T::Output: Clone,
{
    type Output = T::Output;

    fn parse_value(&self, value: &Value) -> Result<T::Output, VldError> {
        if value.is_null() {
            return Ok(self.default_value.clone());
        }
        self.inner.parse_value(value)
    }
}
