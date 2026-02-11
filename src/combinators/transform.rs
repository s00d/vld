use serde_json::Value;
use std::marker::PhantomData;

use crate::error::VldError;
use crate::schema::VldSchema;

/// Transforms the output of a schema after successful parsing.
///
/// Created via [`VldSchema::transform()`].
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// // Parse a string, then transform it to its length
/// let len_schema = vld::string().transform(|s| s.len());
/// ```
pub struct ZTransform<T, F, U>
where
    T: VldSchema,
    F: Fn(T::Output) -> U,
{
    inner: T,
    transform_fn: F,
    _phantom: PhantomData<U>,
}

impl<T, F, U> ZTransform<T, F, U>
where
    T: VldSchema,
    F: Fn(T::Output) -> U,
{
    pub fn new(inner: T, transform_fn: F) -> Self {
        Self {
            inner,
            transform_fn,
            _phantom: PhantomData,
        }
    }

    /// Access the inner schema.
    pub fn inner_schema(&self) -> &T {
        &self.inner
    }
}

impl<T, F, U> VldSchema for ZTransform<T, F, U>
where
    T: VldSchema,
    F: Fn(T::Output) -> U,
{
    type Output = U;

    fn parse_value(&self, value: &Value) -> Result<U, VldError> {
        let result = self.inner.parse_value(value)?;
        Ok((self.transform_fn)(result))
    }
}
