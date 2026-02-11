use serde_json::Value;

use crate::error::VldError;
use crate::schema::VldSchema;

/// Wraps a schema to make it both optional and nullable (nullish).
///
/// Equivalent to `.optional()` + `.nullable()` in Zod.
/// If the value is `null` or missing, returns `Ok(None)`.
pub struct ZNullish<T: VldSchema> {
    inner: T,
}

impl<T: VldSchema> ZNullish<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }

    /// Access the inner schema (for JSON Schema generation).
    pub fn inner_schema(&self) -> &T {
        &self.inner
    }
}

impl<T: VldSchema> VldSchema for ZNullish<T> {
    type Output = Option<T::Output>;

    fn parse_value(&self, value: &Value) -> Result<Option<T::Output>, VldError> {
        if value.is_null() {
            return Ok(None);
        }
        self.inner.parse_value(value).map(Some)
    }
}
