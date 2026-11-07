use serde_json::Value;

use crate::error::VldError;
use crate::schema::VldSchema;

/// Wraps a schema with a fallback value returned on ANY validation error.
///
/// Unlike [`ZDefault`](crate::modifiers::ZDefault) which only catches null/missing,
/// `ZCatch` catches all errors including invalid data.
///
/// Created via [`VldSchema::catch()`].
pub struct ZCatch<T: VldSchema>
where
    T::Output: Clone,
{
    inner: T,
    fallback: T::Output,
}

impl<T: VldSchema> ZCatch<T>
where
    T::Output: Clone,
{
    /// Create the schema (the same as its `vld::` constructor function).
    pub fn new(inner: T, fallback: T::Output) -> Self {
        Self { inner, fallback }
    }

    /// Access the inner schema.
    pub fn inner_schema(&self) -> &T {
        &self.inner
    }
}

impl<T: VldSchema> VldSchema for ZCatch<T>
where
    T::Output: Clone,
{
    type Output = T::Output;

    fn parse_value(&self, value: &Value) -> Result<T::Output, VldError> {
        Ok(self
            .inner
            .parse_value(value)
            .unwrap_or_else(|_| self.fallback.clone()))
    }
    fn parse_missing(&self) -> Result<T::Output, VldError> {
        Ok(self
            .inner
            .parse_missing()
            .unwrap_or_else(|_| self.fallback.clone()))
    }
    fn output_json(&self, o: &Self::Output) -> Option<Value> {
        self.inner.output_json(o)
    }
}
