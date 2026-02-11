use serde_json::Value;

use crate::error::VldError;
use crate::schema::VldSchema;

/// Override the error message for an inner schema.
///
/// Created via [`VldSchema::message()`]. On validation failure the **first**
/// issue's message is replaced; if the schema produces multiple issues they
/// are all replaced.
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// let schema = vld::string().min(3).message("Too short");
/// let err = schema.parse(r#""ab""#).unwrap_err();
/// assert_eq!(err.issues[0].message, "Too short");
/// ```
pub struct ZMessage<T: VldSchema> {
    inner: T,
    msg: String,
}

impl<T: VldSchema> ZMessage<T> {
    pub fn new(inner: T, msg: impl Into<String>) -> Self {
        Self {
            inner,
            msg: msg.into(),
        }
    }
}

impl<T: VldSchema> VldSchema for ZMessage<T> {
    type Output = T::Output;

    fn parse_value(&self, value: &Value) -> Result<T::Output, VldError> {
        self.inner.parse_value(value).map_err(|mut err| {
            for issue in &mut err.issues {
                issue.message = self.msg.clone();
            }
            err
        })
    }
}
