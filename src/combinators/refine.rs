use serde_json::Value;

use crate::error::{IssueCode, VldError};
use crate::schema::VldSchema;

/// Adds a custom refinement check to a schema without changing its output type.
///
/// Created via [`VldSchema::refine()`].
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// let even = vld::number().int().refine(|n| n % 2 == 0, "Must be even");
/// ```
pub struct ZRefine<T, F>
where
    T: VldSchema,
    F: Fn(&T::Output) -> bool,
{
    inner: T,
    check: F,
    message: String,
}

impl<T, F> ZRefine<T, F>
where
    T: VldSchema,
    F: Fn(&T::Output) -> bool,
{
    pub fn new(inner: T, check: F, message: &str) -> Self {
        Self {
            inner,
            check,
            message: message.to_string(),
        }
    }

    /// Access the inner schema.
    pub fn inner_schema(&self) -> &T {
        &self.inner
    }
}

impl<T, F> VldSchema for ZRefine<T, F>
where
    T: VldSchema,
    F: Fn(&T::Output) -> bool,
{
    type Output = T::Output;

    fn parse_value(&self, value: &Value) -> Result<T::Output, VldError> {
        let result = self.inner.parse_value(value)?;

        if (self.check)(&result) {
            Ok(result)
        } else {
            Err(VldError::single(
                IssueCode::Custom {
                    code: "custom".to_string(),
                },
                self.message.clone(),
            ))
        }
    }
}
