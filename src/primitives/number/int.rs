use serde_json::Value;

use crate::error::{issue_error, IssueCode, SizeOrigin, VldError};
use crate::schema::VldSchema;

use super::{Num, ZNumber};

/// Schema for integer validation (`i64`). Created via `vld::number().int()`.
///
/// Integers are read exactly (no `f64` round-trip), so values above `2^53`
/// keep their precision; inputs outside the `i64` range are rejected.
#[derive(Clone)]
pub struct ZInt {
    inner: ZNumber,
    custom_int_error: Option<String>,
}

impl ZInt {
    pub(super) fn new(inner: ZNumber) -> Self {
        Self {
            inner,
            custom_int_error: None,
        }
    }

    /// Set a custom error message for type mismatch (non-number input).
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.inner = self.inner.type_error(msg);
        self
    }

    /// Set a custom error message for when the number is not an integer.
    ///
    /// # Example
    /// ```
    /// use vld::prelude::*;
    /// let schema = vld::number().int().int_error("Whole numbers only!");
    /// let err = schema.parse("3.5").unwrap_err();
    /// assert!(err.issues[0].message.contains("Whole numbers only!"));
    /// ```
    pub fn int_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_int_error = Some(msg.into());
        self
    }

    /// Override error messages in bulk by check key.
    ///
    /// Same keys as [`ZNumber::with_messages`], plus `"not_int"` for the
    /// integer check itself.
    pub fn with_messages<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        if let Some(msg) = f("not_int") {
            self.custom_int_error = Some(msg);
        }
        self.inner = self.inner.with_messages(f);
        self
    }

    /// Coerce strings (`"42"`) and booleans to integers.
    pub fn coerce(mut self) -> Self {
        self.inner.coerce = true;
        self
    }

    /// Minimum value (inclusive).
    pub fn min(mut self, val: i64) -> Self {
        self.inner = self.inner.min_n(Num::I(val));
        self
    }

    /// Maximum value (inclusive).
    pub fn max(mut self, val: i64) -> Self {
        self.inner = self.inner.max_n(Num::I(val));
        self
    }

    /// Greater than (exclusive).
    pub fn gt(mut self, val: i64) -> Self {
        self.inner = self.inner.gt_n(Num::I(val));
        self
    }

    /// Greater than or equal (inclusive). Same as `min`.
    pub fn gte(self, val: i64) -> Self {
        self.min(val)
    }

    /// Less than (exclusive).
    pub fn lt(mut self, val: i64) -> Self {
        self.inner = self.inner.lt_n(Num::I(val));
        self
    }

    /// Less than or equal (inclusive). Same as `max`.
    pub fn lte(self, val: i64) -> Self {
        self.max(val)
    }

    /// Must be positive (> 0).
    pub fn positive(mut self) -> Self {
        self.inner = self.inner.positive();
        self
    }

    /// Must be negative (< 0).
    pub fn negative(mut self) -> Self {
        self.inner = self.inner.negative();
        self
    }

    /// Must be non-negative (>= 0).
    pub fn non_negative(mut self) -> Self {
        self.inner = self.inner.non_negative();
        self
    }

    /// Must be non-positive (<= 0).
    pub fn non_positive(mut self) -> Self {
        self.inner = self.inner.non_positive();
        self
    }

    /// Must be within JavaScript's safe integer range.
    pub fn safe(mut self) -> Self {
        self.inner = self.inner.safe();
        self
    }

    /// Must be a multiple of the given value (`0` never matches).
    pub fn multiple_of(mut self, val: i64) -> Self {
        self.inner = self.inner.multiple_of_n(Num::I(val));
        self
    }

    /// Generate a JSON Schema representation of this integer schema.
    ///
    /// Requires the `openapi` feature.
    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        let mut schema = self.inner.to_json_schema();
        schema["type"] = serde_json::json!("integer");
        schema
    }

    fn not_int(&self, value: &Value) -> VldError {
        issue_error(IssueCode::NotInt, self.custom_int_error.as_ref(), value)
    }

    fn int_from_f64(&self, f: f64, value: &Value) -> Result<i64, VldError> {
        if f.fract() != 0.0 {
            return Err(self.not_int(value));
        }
        // `i64::MAX as f64` rounds up to 2^63, which is itself out of range.
        if f < i64::MIN as f64 || f >= i64::MAX as f64 {
            return Err(out_of_range(f < 0.0, value));
        }
        Ok(f as i64)
    }

    fn extract_int(&self, value: &Value) -> Result<i64, VldError> {
        match value {
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(i)
                } else if n.as_u64().is_some() {
                    Err(out_of_range(false, value))
                } else {
                    self.int_from_f64(n.as_f64().unwrap_or(f64::NAN), value)
                }
            }
            Value::String(s) if self.inner.coerce => match s.parse::<i64>() {
                Ok(i) => Ok(i),
                Err(_) => {
                    let f = self.inner.extract_number(value)?;
                    self.int_from_f64(f, value)
                }
            },
            _ => self.inner.extract_number(value).map(|f| f as i64),
        }
    }
}

fn out_of_range(negative: bool, value: &Value) -> VldError {
    let code = if negative {
        IssueCode::TooSmall {
            origin: SizeOrigin::Number,
            minimum: i64::MIN.into(),
            inclusive: true,
        }
    } else {
        IssueCode::TooBig {
            origin: SizeOrigin::Number,
            maximum: i64::MAX.into(),
            inclusive: true,
        }
    };
    VldError::from_code_with_value(code, value)
}

impl VldSchema for ZInt {
    type Output = i64;

    fn parse_value(&self, value: &Value) -> Result<i64, VldError> {
        let n = self.extract_int(value)?;
        self.inner.validate(Num::I(n), value)?;
        Ok(n)
    }
}
