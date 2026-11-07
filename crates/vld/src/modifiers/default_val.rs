use serde_json::Value;

use crate::error::VldError;
use crate::schema::VldSchema;

/// Wraps a schema to provide a default value when the input is `null` or missing
/// (and, with [`blank_as`](crate::schema::VldSchema::blank_as), a blank string).
///
/// - If the value is `null` → returns the default value.
/// - If the value is present and valid → returns the parsed value.
/// - If the value is present but invalid → returns an error (NOT the default).
pub struct ZDefault<T: VldSchema>
where
    T::Output: Clone,
{
    inner: T,
    default_value: T::Output,
    /// Also blank strings (`blank_as`).
    blank: bool,
}

impl<T: VldSchema> ZDefault<T>
where
    T::Output: Clone,
{
    /// Create the schema (the same as its `vld::` constructor function).
    pub fn new(inner: T, default_value: T::Output) -> Self {
        Self {
            inner,
            default_value,
            blank: false,
        }
    }

    /// Also take the default for a blank string (empty or whitespace only).
    pub(crate) fn or_blank(mut self) -> Self {
        self.blank = true;
        self
    }

    /// `true` for [`blank_as`](crate::schema::VldSchema::blank_as).
    pub fn accepts_blank(&self) -> bool {
        self.blank
    }

    /// Access the inner schema (for JSON Schema generation).
    pub fn inner_schema(&self) -> &T {
        &self.inner
    }

    /// Access the default value (emitted as `"default"` in JSON Schema).
    pub fn default_value(&self) -> &T::Output {
        &self.default_value
    }
}

impl<T: VldSchema> VldSchema for ZDefault<T>
where
    T::Output: Clone,
{
    type Output = T::Output;

    fn parse_value(&self, value: &Value) -> Result<T::Output, VldError> {
        let blank = self.blank && value.as_str().is_some_and(|s| s.trim().is_empty());
        if value.is_null() || blank {
            return Ok(self.default_value.clone());
        }
        self.inner.parse_value(value)
    }
    fn output_json(&self, o: &Self::Output) -> Option<Value> {
        self.inner.output_json(o)
    }
}
