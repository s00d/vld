use serde_json::Value;

use crate::error::VldError;
use crate::schema::VldSchema;

/// Refinement that can produce multiple errors at once.
///
/// Created via [`VldSchema::super_refine()`].
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// let schema = vld::string().super_refine(|s, errors| {
///     if s.len() < 3 {
///         errors.push(IssueCode::Custom { code: "too_short".into() }, "Too short");
///     }
///     if !s.contains('@') {
///         errors.push(IssueCode::Custom { code: "no_at".into() }, "Missing @");
///     }
/// });
/// ```
pub struct ZSuperRefine<T, F>
where
    T: VldSchema,
    F: Fn(&T::Output, &mut VldError),
{
    inner: T,
    check: F,
}

impl<T, F> ZSuperRefine<T, F>
where
    T: VldSchema,
    F: Fn(&T::Output, &mut VldError),
{
    pub fn new(inner: T, check: F) -> Self {
        Self { inner, check }
    }
}

impl<T, F> VldSchema for ZSuperRefine<T, F>
where
    T: VldSchema,
    F: Fn(&T::Output, &mut VldError),
{
    type Output = T::Output;

    fn parse_value(&self, value: &Value) -> Result<T::Output, VldError> {
        let result = self.inner.parse_value(value)?;
        let mut errors = VldError::new();
        (self.check)(&result, &mut errors);
        if errors.is_empty() {
            Ok(result)
        } else {
            Err(errors)
        }
    }
}
