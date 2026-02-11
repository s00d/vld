use serde_json::Value;

use crate::error::VldError;
use crate::schema::VldSchema;

/// Wraps a schema to allow null values.
///
/// If the value is `null`, returns `Ok(None)`.
/// Otherwise, delegates to the inner schema and wraps the result in `Some`.
///
/// The difference from [`ZOptional`](super::ZOptional) is semantic:
/// - `optional()`: field can be missing entirely
/// - `nullable()`: field must be present but can be null
///
/// At the `Value` level both behave the same. The distinction matters
/// when used with the `schema!` macro for object validation.
pub struct ZNullable<T: VldSchema> {
    inner: T,
}

impl<T: VldSchema> ZNullable<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }

    /// Access the inner schema (for JSON Schema generation).
    pub fn inner_schema(&self) -> &T {
        &self.inner
    }
}

impl<T: VldSchema> VldSchema for ZNullable<T> {
    type Output = Option<T::Output>;

    fn parse_value(&self, value: &Value) -> Result<Option<T::Output>, VldError> {
        if value.is_null() {
            return Ok(None);
        }
        self.inner.parse_value(value).map(Some)
    }
}
