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
        value.as_i64()
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
pub struct ZLiteral<T: IntoLiteral> {
    expected_value: Value,
    display: String,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: IntoLiteral> ZLiteral<T> {
    pub fn new(expected: T) -> Self {
        let expected_value = expected.to_json_value();
        let display = expected.display();
        Self {
            expected_value,
            display,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T: IntoLiteral> VldSchema for ZLiteral<T> {
    type Output = T::Output;

    fn parse_value(&self, value: &Value) -> Result<T::Output, VldError> {
        if *value == self.expected_value {
            T::extract(value).ok_or_else(|| {
                VldError::single(
                    IssueCode::Custom {
                        code: "invalid_literal".to_string(),
                    },
                    format!("Expected literal {}", self.display),
                )
            })
        } else {
            Err(VldError::single_with_value(
                IssueCode::Custom {
                    code: "invalid_literal".to_string(),
                },
                format!("Expected literal {}, received {:?}", self.display, value),
                value,
            ))
        }
    }
}
