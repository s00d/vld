use serde_json::Value;

use crate::error::VldError;
use crate::schema::VldSchema;

/// Applies a preprocessing function to the JSON value before validation.
///
/// Created via [`vld::preprocess()`](crate::preprocess).
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// // Trim whitespace from string values before validating
/// let schema = vld::preprocess(
///     |v| match v.as_str() {
///         Some(s) => serde_json::Value::String(s.trim().to_string()),
///         None => v.clone(),
///     },
///     vld::string().min(1),
/// );
/// ```
pub struct ZPreprocess<F, S>
where
    F: Fn(&Value) -> Value,
    S: VldSchema,
{
    preprocessor: F,
    schema: S,
}

impl<F, S> ZPreprocess<F, S>
where
    F: Fn(&Value) -> Value,
    S: VldSchema,
{
    pub fn new(preprocessor: F, schema: S) -> Self {
        Self {
            preprocessor,
            schema,
        }
    }
}

impl<F, S> VldSchema for ZPreprocess<F, S>
where
    F: Fn(&Value) -> Value,
    S: VldSchema,
{
    type Output = S::Output;

    fn parse_value(&self, value: &Value) -> Result<S::Output, VldError> {
        let preprocessed = (self.preprocessor)(value);
        self.schema.parse_value(&preprocessed)
    }
}
