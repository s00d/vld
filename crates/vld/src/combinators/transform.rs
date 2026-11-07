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
    /// Create the schema (the same as its `vld::` constructor function).
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
        Ok((self.transform_fn)(self.inner.parse_value(value)?))
    }
    fn parse_missing(&self) -> Result<U, VldError> {
        Ok((self.transform_fn)(self.inner.parse_missing()?))
    }
}

/// Transforms the output with a fallible function: an `Err` becomes a
/// validation issue (`IssueCode::Custom { code: "transform" }`, the error's
/// text as the message).
///
/// Created via [`VldSchema::try_transform()`].
///
/// ```
/// use vld::prelude::*;
///
/// let port = vld::string().try_transform(|s| s.parse::<u16>());
/// assert_eq!(port.parse(r#""8080""#).unwrap(), 8080);
/// let err = port.parse(r#""99999""#).unwrap_err();
/// assert_eq!(err.issues[0].code.key(), "transform");
/// assert_eq!(err.issues[0].message, "number too large to fit in target type");
/// ```
pub struct ZTryTransform<T, F, U> {
    inner: T,
    transform_fn: F,
    _phantom: PhantomData<U>,
}

impl<T, F, U> ZTryTransform<T, F, U> {
    /// Create the schema (the same as [`VldSchema::try_transform`]).
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

impl<T, F, U, E> ZTryTransform<T, F, U>
where
    T: VldSchema,
    F: Fn(T::Output) -> Result<U, E>,
    E: std::fmt::Display,
{
    fn apply(&self, out: T::Output) -> Result<U, VldError> {
        (self.transform_fn)(out).map_err(|e| {
            VldError::single(
                crate::error::IssueCode::Custom {
                    code: "transform".to_string(),
                },
                e.to_string(),
            )
        })
    }
}

impl<T, F, U, E> VldSchema for ZTryTransform<T, F, U>
where
    T: VldSchema,
    F: Fn(T::Output) -> Result<U, E>,
    E: std::fmt::Display,
{
    type Output = U;

    fn parse_value(&self, value: &Value) -> Result<U, VldError> {
        self.apply(self.inner.parse_value(value)?)
    }
    fn parse_missing(&self) -> Result<U, VldError> {
        self.apply(self.inner.parse_missing()?)
    }
}
