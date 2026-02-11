use serde_json::Value;
use std::marker::PhantomData;

use crate::error::{IssueCode, VldError};
use crate::schema::VldSchema;

/// Schema from a custom validation function.
///
/// Created via [`vld::custom()`](crate::custom).
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// let even = vld::custom(|v: &serde_json::Value| {
///     let n = v.as_i64().ok_or("Expected integer")?;
///     if n % 2 == 0 { Ok(n) } else { Err("Must be even".into()) }
/// });
/// assert!(even.parse("4").is_ok());
/// assert!(even.parse("5").is_err());
/// ```
pub struct ZCustom<F, T> {
    check: F,
    _marker: PhantomData<T>,
}

impl<F, T> ZCustom<F, T>
where
    F: Fn(&Value) -> Result<T, String>,
{
    pub fn new(check: F) -> Self {
        Self {
            check,
            _marker: PhantomData,
        }
    }
}

impl<F, T> VldSchema for ZCustom<F, T>
where
    F: Fn(&Value) -> Result<T, String>,
{
    type Output = T;

    fn parse_value(&self, value: &Value) -> Result<T, VldError> {
        (self.check)(value).map_err(|msg| {
            VldError::single_with_value(
                IssueCode::Custom {
                    code: "custom".to_string(),
                },
                msg,
                value,
            )
        })
    }
}
