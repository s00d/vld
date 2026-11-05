use serde_json::Value;
use std::cmp::Ordering;

use crate::error::{IssueCode, VldError};
use crate::schema::VldSchema;

use super::{Kind, Num, NumberCheck, ZInt};

/// Schema for number validation (`f64`). Created via [`vld::number()`](crate::number).
///
/// Use `.int()` to convert to integer validation (`i64`).
///
/// `NaN` never satisfies a bound check, and `.coerce()` rejects strings that
/// parse to `NaN` or infinity.
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// let schema = vld::number().min(0.0).max(100.0);
/// let int_schema = vld::number().int().min(0).max(100);
/// ```
#[derive(Clone)]
pub struct ZNumber {
    checks: Vec<NumberCheck>,
    pub(super) coerce: bool,
    custom_type_error: Option<String>,
}

impl ZNumber {
    pub fn new() -> Self {
        Self {
            checks: vec![],
            coerce: false,
            custom_type_error: None,
        }
    }

    fn push(mut self, kind: Kind, msg: impl Into<String>) -> Self {
        self.checks.push(NumberCheck {
            kind,
            msg: msg.into(),
        });
        self
    }

    /// Set a custom error message for type mismatch (when the input is not a number).
    ///
    /// # Example
    /// ```
    /// use vld::prelude::*;
    /// let schema = vld::number().type_error("Must be a number!");
    /// let err = schema.parse(r#""hello""#).unwrap_err();
    /// assert!(err.issues[0].message.contains("Must be a number!"));
    /// ```
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    /// Override error messages in bulk by check key.
    ///
    /// The closure receives the check key (e.g. `"too_small"`, `"too_big"`,
    /// `"not_positive"`, `"not_finite"`, `"not_multiple_of"`, `"not_safe"`)
    /// and should return `Some(new_message)` to replace, or `None` to keep the original.
    ///
    /// # Example
    /// ```
    /// use vld::prelude::*;
    /// let schema = vld::number().min(0.0).max(100.0)
    ///     .with_messages(|key| match key {
    ///         "too_small" => Some("Must be >= 0".into()),
    ///         "too_big" => Some("Must be <= 100".into()),
    ///         _ => None,
    ///     });
    /// ```
    pub fn with_messages<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        for check in &mut self.checks {
            if let Some(msg) = f(check.kind.key()) {
                check.msg = msg;
            }
        }
        self
    }

    pub(super) fn min_n(self, v: Num) -> Self {
        self.push(Kind::Min(v), format!("Number must be at least {}", v))
    }

    pub(super) fn max_n(self, v: Num) -> Self {
        self.push(Kind::Max(v), format!("Number must be at most {}", v))
    }

    pub(super) fn gt_n(self, v: Num) -> Self {
        self.push(Kind::Gt(v), format!("Number must be greater than {}", v))
    }

    pub(super) fn lt_n(self, v: Num) -> Self {
        self.push(Kind::Lt(v), format!("Number must be less than {}", v))
    }

    pub(super) fn multiple_of_n(self, v: Num) -> Self {
        self.push(
            Kind::MultipleOf(v),
            format!("Number must be a multiple of {}", v),
        )
    }

    /// Minimum value (inclusive). Alias: `gte`.
    pub fn min(self, val: f64) -> Self {
        self.min_n(Num::F(val))
    }

    /// Maximum value (inclusive). Alias: `lte`.
    pub fn max(self, val: f64) -> Self {
        self.max_n(Num::F(val))
    }

    /// Greater than (exclusive).
    pub fn gt(self, val: f64) -> Self {
        self.gt_n(Num::F(val))
    }

    /// Greater than or equal (inclusive). Same as `min`.
    pub fn gte(self, val: f64) -> Self {
        self.min(val)
    }

    /// Less than (exclusive).
    pub fn lt(self, val: f64) -> Self {
        self.lt_n(Num::F(val))
    }

    /// Less than or equal (inclusive). Same as `max`.
    pub fn lte(self, val: f64) -> Self {
        self.max(val)
    }

    /// Must be positive (> 0).
    pub fn positive(self) -> Self {
        self.push(Kind::Positive, "Number must be positive")
    }

    /// Must be negative (< 0).
    pub fn negative(self) -> Self {
        self.push(Kind::Negative, "Number must be negative")
    }

    /// Must be non-negative (>= 0).
    pub fn non_negative(self) -> Self {
        self.push(Kind::NonNegative, "Number must be non-negative")
    }

    /// Must be non-positive (<= 0).
    pub fn non_positive(self) -> Self {
        self.push(Kind::NonPositive, "Number must be non-positive")
    }

    /// Must be finite (not NaN or infinity).
    pub fn finite(self) -> Self {
        self.push(Kind::Finite, "Number must be finite")
    }

    /// Must be a multiple of the given value.
    ///
    /// Uses a relative tolerance of `1e-9` so decimal steps like `0.1` work.
    /// A divisor of `0` never matches (every value is rejected).
    pub fn multiple_of(self, val: f64) -> Self {
        self.multiple_of_n(Num::F(val))
    }

    /// Must be within JavaScript's safe integer range (`-(2^53 - 1)` to `2^53 - 1`).
    pub fn safe(self) -> Self {
        self.push(
            Kind::Safe,
            "Number must be a safe integer (-(2^53-1) to 2^53-1)",
        )
    }

    /// Convert to integer validation. Returns `ZInt` with `Output = i64`.
    pub fn int(self) -> ZInt {
        ZInt::new(self)
    }

    /// Coerce strings and booleans to numbers.
    ///
    /// Strings that parse to `NaN` or infinity are rejected.
    pub fn coerce(mut self) -> Self {
        self.coerce = true;
        self
    }

    fn type_err(&self, value: &Value) -> VldError {
        crate::primitives::type_error(&self.custom_type_error, "number", "number", value)
    }

    fn coerce_err(&self, s: &str, value: &Value) -> VldError {
        let msg = self
            .custom_type_error
            .clone()
            .unwrap_or_else(|| format!("Cannot coerce \"{}\" to number", s));
        VldError::single_with_value(
            IssueCode::InvalidType {
                expected: "number".to_string(),
                received: "string".to_string(),
            },
            msg,
            value,
        )
    }

    pub(super) fn extract_number(&self, value: &Value) -> Result<f64, VldError> {
        match value {
            Value::Number(n) => n.as_f64().ok_or_else(|| self.type_err(value)),
            Value::String(s) if self.coerce => s
                .parse::<f64>()
                .ok()
                .filter(|f| f.is_finite())
                .ok_or_else(|| self.coerce_err(s, value)),
            Value::Bool(b) if self.coerce => Ok(if *b { 1.0 } else { 0.0 }),
            _ => Err(self.type_err(value)),
        }
    }

    pub(super) fn validate(&self, n: Num, value: &Value) -> Result<(), VldError> {
        let mut errors = VldError::new();
        for check in &self.checks {
            if let Some(code) = check.kind.failure(n) {
                errors.push_with_value(code, check.msg.clone(), value);
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Generate a JSON Schema representation of this number schema.
    ///
    /// The strictest lower/upper bound (from `min`/`gt`/`positive`/
    /// `non_negative`/`safe`, …) is emitted as `minimum` or `exclusiveMinimum`
    /// (likewise for the maximum). The first `multiple_of` goes into
    /// `multipleOf`, further ones into `allOf`.
    ///
    /// Requires the `openapi` feature.
    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        let mut schema = serde_json::json!({"type": "number"});
        // Strictest bound: larger lower bound wins, exclusive wins ties.
        let pick = |a: (Num, bool), b: (Num, bool), want: Ordering| match b.0.cmp(a.0) {
            Some(o) if o == want => b,
            Some(Ordering::Equal) if b.1 => b,
            _ => a,
        };
        let lower = self
            .checks
            .iter()
            .filter_map(|c| c.kind.lower())
            .reduce(|a, b| pick(a, b, Ordering::Greater));
        let upper = self
            .checks
            .iter()
            .filter_map(|c| c.kind.upper())
            .reduce(|a, b| pick(a, b, Ordering::Less));
        if let Some((v, excl)) = lower {
            let key = if excl { "exclusiveMinimum" } else { "minimum" };
            schema[key] = v.json();
        }
        if let Some((v, excl)) = upper {
            let key = if excl { "exclusiveMaximum" } else { "maximum" };
            schema[key] = v.json();
        }
        let mut multiples = self.checks.iter().filter_map(|c| match c.kind {
            Kind::MultipleOf(v) => Some(v.json()),
            _ => None,
        });
        if let Some(m) = multiples.next() {
            schema["multipleOf"] = m;
        }
        let rest: Vec<Value> = multiples
            .map(|m| serde_json::json!({ "multipleOf": m }))
            .collect();
        if !rest.is_empty() {
            schema["allOf"] = Value::Array(rest);
        }
        schema
    }
}

impl Default for ZNumber {
    fn default() -> Self {
        Self::new()
    }
}

impl VldSchema for ZNumber {
    type Output = f64;

    fn parse_value(&self, value: &Value) -> Result<f64, VldError> {
        let n = self.extract_number(value)?;
        self.validate(Num::F(n), value)?;
        Ok(n)
    }
}
