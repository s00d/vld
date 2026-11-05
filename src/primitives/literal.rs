use serde_json::Value;

use crate::error::{IssueCode, VldError};
use crate::schema::VldSchema;

/// Trait for types that can be used as literal values.
pub trait IntoLiteral: Clone + 'static {
    type Output: Clone + PartialEq + std::fmt::Debug;
    fn to_json_value(&self) -> Value;
    fn extract(value: &Value) -> Option<Self::Output>;
    fn display(&self) -> String;
}

impl IntoLiteral for &'static str {
    type Output = String;
    fn to_json_value(&self) -> Value {
        Value::String(self.to_string())
    }
    fn extract(value: &Value) -> Option<String> {
        value.as_str().map(|s| s.to_string())
    }
    fn display(&self) -> String {
        format!("\"{}\"", self)
    }
}

impl IntoLiteral for String {
    type Output = String;
    fn to_json_value(&self) -> Value {
        Value::String(self.clone())
    }
    fn extract(value: &Value) -> Option<String> {
        value.as_str().map(|s| s.to_string())
    }
    fn display(&self) -> String {
        format!("\"{}\"", self)
    }
}

impl IntoLiteral for i64 {
    type Output = i64;
    fn to_json_value(&self) -> Value {
        Value::Number((*self).into())
    }
    fn extract(value: &Value) -> Option<i64> {
        // Accept integral floats (`5.0`) as well as integers.
        value.as_i64().or_else(|| {
            value
                .as_f64()
                .filter(|f| f.fract() == 0.0 && *f >= i64::MIN as f64 && *f < i64::MAX as f64)
                .map(|f| f as i64)
        })
    }
    fn display(&self) -> String {
        self.to_string()
    }
}

impl IntoLiteral for f64 {
    type Output = f64;
    fn to_json_value(&self) -> Value {
        serde_json::Number::from_f64(*self)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
    fn extract(value: &Value) -> Option<f64> {
        value.as_f64()
    }
    fn display(&self) -> String {
        self.to_string()
    }
}

impl IntoLiteral for bool {
    type Output = bool;
    fn to_json_value(&self) -> Value {
        Value::Bool(*self)
    }
    fn extract(value: &Value) -> Option<bool> {
        value.as_bool()
    }
    fn display(&self) -> String {
        self.to_string()
    }
}

/// Schema for exact value matching. Created via [`vld::literal()`](crate::literal).
///
/// Numbers compare by value: `literal(1.0)` accepts `1`, `literal(5i64)` accepts `5.0`.
pub struct ZLiteral<T: IntoLiteral> {
    expected_value: Value,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: IntoLiteral> ZLiteral<T> {
    pub fn new(expected: T) -> Self {
        Self {
            expected_value: expected.to_json_value(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// The literal as a JSON value (for JSON Schema `const`).
    pub fn expected_value(&self) -> &Value {
        &self.expected_value
    }
}

impl<T: IntoLiteral> VldSchema for ZLiteral<T> {
    type Output = T::Output;

    fn parse_value(&self, value: &Value) -> Result<T::Output, VldError> {
        super::json_eq(value, &self.expected_value)
            .then(|| T::extract(value))
            .flatten()
            .ok_or_else(|| {
                VldError::from_code_with_value(
                    IssueCode::InvalidLiteral {
                        expected: self.expected_value.clone(),
                    },
                    value,
                )
            })
    }
}
