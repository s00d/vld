use serde_json::Value;

use crate::error::{value_type_name, IssueCode, VldError};
use crate::schema::VldSchema;

#[derive(Clone)]
enum NumberCheck {
    Min(f64, String),
    Max(f64, String),
    Gt(f64, String),
    Lt(f64, String),
    Positive(String),
    Negative(String),
    NonNegative(String),
    NonPositive(String),
    Finite(String),
    MultipleOf(f64, String),
    Safe(String),
}

impl NumberCheck {
    /// Stable key identifying the check category.
    fn key(&self) -> &str {
        match self {
            NumberCheck::Min(..) => "too_small",
            NumberCheck::Max(..) => "too_big",
            NumberCheck::Gt(..) => "too_small",
            NumberCheck::Lt(..) => "too_big",
            NumberCheck::Positive(..) => "not_positive",
            NumberCheck::Negative(..) => "not_negative",
            NumberCheck::NonNegative(..) => "not_non_negative",
            NumberCheck::NonPositive(..) => "not_non_positive",
            NumberCheck::Finite(..) => "not_finite",
            NumberCheck::MultipleOf(..) => "not_multiple_of",
            NumberCheck::Safe(..) => "not_safe",
        }
    }

    /// Replace the error message stored in this check.
    fn set_message(&mut self, msg: String) {
        match self {
            NumberCheck::Min(_, ref mut m)
            | NumberCheck::Max(_, ref mut m)
            | NumberCheck::Gt(_, ref mut m)
            | NumberCheck::Lt(_, ref mut m)
            | NumberCheck::Positive(ref mut m)
            | NumberCheck::Negative(ref mut m)
            | NumberCheck::NonNegative(ref mut m)
            | NumberCheck::NonPositive(ref mut m)
            | NumberCheck::Finite(ref mut m)
            | NumberCheck::MultipleOf(_, ref mut m)
            | NumberCheck::Safe(ref mut m) => *m = msg,
        }
    }
}

/// Schema for number validation (`f64`). Created via [`vld::number()`](crate::number).
///
/// Use `.int()` to convert to integer validation (`i64`).
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
    coerce: bool,
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
            if let Some(msg) = f(check.key()) {
                check.set_message(msg);
            }
        }
        self
    }

    /// Minimum value (inclusive). Alias: `gte`.
    pub fn min(mut self, val: f64) -> Self {
        self.checks.push(NumberCheck::Min(
            val,
            format!("Number must be at least {}", val),
        ));
        self
    }

    /// Maximum value (inclusive). Alias: `lte`.
    pub fn max(mut self, val: f64) -> Self {
        self.checks.push(NumberCheck::Max(
            val,
            format!("Number must be at most {}", val),
        ));
        self
    }

    /// Greater than (exclusive).
    pub fn gt(mut self, val: f64) -> Self {
        self.checks.push(NumberCheck::Gt(
            val,
            format!("Number must be greater than {}", val),
        ));
        self
    }

    /// Greater than or equal (inclusive). Same as `min`.
    pub fn gte(self, val: f64) -> Self {
        self.min(val)
    }

    /// Less than (exclusive).
    pub fn lt(mut self, val: f64) -> Self {
        self.checks.push(NumberCheck::Lt(
            val,
            format!("Number must be less than {}", val),
        ));
        self
    }

    /// Less than or equal (inclusive). Same as `max`.
    pub fn lte(self, val: f64) -> Self {
        self.max(val)
    }

    /// Must be positive (> 0).
    pub fn positive(mut self) -> Self {
        self.checks
            .push(NumberCheck::Positive("Number must be positive".to_string()));
        self
    }

    /// Must be negative (< 0).
    pub fn negative(mut self) -> Self {
        self.checks
            .push(NumberCheck::Negative("Number must be negative".to_string()));
        self
    }

    /// Must be non-negative (>= 0).
    pub fn non_negative(mut self) -> Self {
        self.checks.push(NumberCheck::NonNegative(
            "Number must be non-negative".to_string(),
        ));
        self
    }

    /// Must be non-positive (<= 0).
    pub fn non_positive(mut self) -> Self {
        self.checks.push(NumberCheck::NonPositive(
            "Number must be non-positive".to_string(),
        ));
        self
    }

    /// Must be finite (not NaN or infinity).
    pub fn finite(mut self) -> Self {
        self.checks
            .push(NumberCheck::Finite("Number must be finite".to_string()));
        self
    }

    /// Must be a multiple of the given value.
    pub fn multiple_of(mut self, val: f64) -> Self {
        self.checks.push(NumberCheck::MultipleOf(
            val,
            format!("Number must be a multiple of {}", val),
        ));
        self
    }

    /// Must be within JavaScript's safe integer range (`-(2^53 - 1)` to `2^53 - 1`).
    pub fn safe(mut self) -> Self {
        self.checks.push(NumberCheck::Safe(
            "Number must be a safe integer (-(2^53-1) to 2^53-1)".to_string(),
        ));
        self
    }

    /// Convert to integer validation. Returns `ZInt` with `Output = i64`.
    pub fn int(self) -> ZInt {
        ZInt {
            inner: self,
            custom_int_error: None,
        }
    }

    /// Coerce strings and booleans to numbers.
    pub fn coerce(mut self) -> Self {
        self.coerce = true;
        self
    }

    fn extract_number(&self, value: &Value) -> Result<f64, VldError> {
        let type_err = |value: &Value| -> VldError {
            let msg = self
                .custom_type_error
                .clone()
                .unwrap_or_else(|| format!("Expected number, received {}", value_type_name(value)));
            VldError::single_with_value(
                IssueCode::InvalidType {
                    expected: "number".to_string(),
                    received: value_type_name(value),
                },
                msg,
                value,
            )
        };

        if let Some(n) = value.as_f64() {
            Ok(n)
        } else if self.coerce {
            match value {
                Value::String(s) => s.parse::<f64>().map_err(|_| {
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
                }),
                Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
                _ => Err(type_err(value)),
            }
        } else {
            Err(type_err(value))
        }
    }

    fn validate_number(&self, n: f64, value: &Value) -> Result<f64, VldError> {
        let mut errors = VldError::new();

        for check in &self.checks {
            match check {
                NumberCheck::Min(min, msg) => {
                    if n < *min {
                        errors.push_with_value(
                            IssueCode::TooSmall {
                                minimum: *min,
                                inclusive: true,
                            },
                            msg.clone(),
                            value,
                        );
                    }
                }
                NumberCheck::Max(max, msg) => {
                    if n > *max {
                        errors.push_with_value(
                            IssueCode::TooBig {
                                maximum: *max,
                                inclusive: true,
                            },
                            msg.clone(),
                            value,
                        );
                    }
                }
                NumberCheck::Gt(val, msg) => {
                    if n <= *val {
                        errors.push_with_value(
                            IssueCode::TooSmall {
                                minimum: *val,
                                inclusive: false,
                            },
                            msg.clone(),
                            value,
                        );
                    }
                }
                NumberCheck::Lt(val, msg) => {
                    if n >= *val {
                        errors.push_with_value(
                            IssueCode::TooBig {
                                maximum: *val,
                                inclusive: false,
                            },
                            msg.clone(),
                            value,
                        );
                    }
                }
                NumberCheck::Positive(msg) => {
                    if n <= 0.0 {
                        errors.push_with_value(
                            IssueCode::TooSmall {
                                minimum: 0.0,
                                inclusive: false,
                            },
                            msg.clone(),
                            value,
                        );
                    }
                }
                NumberCheck::Negative(msg) => {
                    if n >= 0.0 {
                        errors.push_with_value(
                            IssueCode::TooBig {
                                maximum: 0.0,
                                inclusive: false,
                            },
                            msg.clone(),
                            value,
                        );
                    }
                }
                NumberCheck::NonNegative(msg) => {
                    if n < 0.0 {
                        errors.push_with_value(
                            IssueCode::TooSmall {
                                minimum: 0.0,
                                inclusive: true,
                            },
                            msg.clone(),
                            value,
                        );
                    }
                }
                NumberCheck::NonPositive(msg) => {
                    if n > 0.0 {
                        errors.push_with_value(
                            IssueCode::TooBig {
                                maximum: 0.0,
                                inclusive: true,
                            },
                            msg.clone(),
                            value,
                        );
                    }
                }
                NumberCheck::Finite(msg) => {
                    if !n.is_finite() {
                        errors.push_with_value(IssueCode::NotFinite, msg.clone(), value);
                    }
                }
                NumberCheck::MultipleOf(val, msg) => {
                    if (n % val).abs() > f64::EPSILON {
                        errors.push_with_value(
                            IssueCode::Custom {
                                code: "not_multiple_of".to_string(),
                            },
                            msg.clone(),
                            value,
                        );
                    }
                }
                NumberCheck::Safe(msg) => {
                    const MAX_SAFE: f64 = 9007199254740991.0;
                    if !(-MAX_SAFE..=MAX_SAFE).contains(&n) {
                        errors.push_with_value(
                            IssueCode::Custom {
                                code: "not_safe".to_string(),
                            },
                            msg.clone(),
                            value,
                        );
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(n)
        } else {
            Err(errors)
        }
    }
}

impl Default for ZNumber {
    fn default() -> Self {
        Self::new()
    }
}

impl ZNumber {
    /// Generate a JSON Schema representation of this number schema.
    ///
    /// Requires the `openapi` feature.
    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        let mut schema = serde_json::json!({"type": "number"});
        for check in &self.checks {
            match check {
                NumberCheck::Min(n, _) => {
                    schema["minimum"] = serde_json::json!(*n);
                }
                NumberCheck::Max(n, _) => {
                    schema["maximum"] = serde_json::json!(*n);
                }
                NumberCheck::Gt(n, _) => {
                    schema["exclusiveMinimum"] = serde_json::json!(*n);
                }
                NumberCheck::Lt(n, _) => {
                    schema["exclusiveMaximum"] = serde_json::json!(*n);
                }
                NumberCheck::MultipleOf(n, _) => {
                    schema["multipleOf"] = serde_json::json!(*n);
                }
                _ => {}
            }
        }
        schema
    }
}

impl VldSchema for ZNumber {
    type Output = f64;

    fn parse_value(&self, value: &Value) -> Result<f64, VldError> {
        let n = self.extract_number(value)?;
        self.validate_number(n, value)
    }
}

// ---------------------------------------------------------------------------
// ZInt — integer validation (i64)
// ---------------------------------------------------------------------------

/// Schema for integer validation (`i64`). Created via `vld::number().int()`.
#[derive(Clone)]
pub struct ZInt {
    inner: ZNumber,
    custom_int_error: Option<String>,
}

impl ZInt {
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

    /// Minimum value (inclusive).
    pub fn min(mut self, val: i64) -> Self {
        self.inner.checks.push(NumberCheck::Min(
            val as f64,
            format!("Number must be at least {}", val),
        ));
        self
    }

    /// Maximum value (inclusive).
    pub fn max(mut self, val: i64) -> Self {
        self.inner.checks.push(NumberCheck::Max(
            val as f64,
            format!("Number must be at most {}", val),
        ));
        self
    }

    /// Greater than (exclusive).
    pub fn gt(mut self, val: i64) -> Self {
        self.inner.checks.push(NumberCheck::Gt(
            val as f64,
            format!("Number must be greater than {}", val),
        ));
        self
    }

    /// Greater than or equal (inclusive). Same as `min`.
    pub fn gte(self, val: i64) -> Self {
        self.min(val)
    }

    /// Less than (exclusive).
    pub fn lt(mut self, val: i64) -> Self {
        self.inner.checks.push(NumberCheck::Lt(
            val as f64,
            format!("Number must be less than {}", val),
        ));
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

    /// Must be within JavaScript's safe integer range.
    pub fn safe(mut self) -> Self {
        self.inner = self.inner.safe();
        self
    }

    /// Must be a multiple of the given value.
    pub fn multiple_of(mut self, val: i64) -> Self {
        self.inner.checks.push(NumberCheck::MultipleOf(
            val as f64,
            format!("Number must be a multiple of {}", val),
        ));
        self
    }
}

impl ZInt {
    /// Generate a JSON Schema representation of this integer schema.
    ///
    /// Requires the `openapi` feature.
    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        let mut schema = self.inner.to_json_schema();
        schema["type"] = serde_json::json!("integer");
        schema
    }
}

impl VldSchema for ZInt {
    type Output = i64;

    fn parse_value(&self, value: &Value) -> Result<i64, VldError> {
        let n = self.inner.extract_number(value)?;

        // Check integer
        if n.fract() != 0.0 {
            let msg = self
                .custom_int_error
                .clone()
                .unwrap_or_else(|| "Expected integer, received float".to_string());
            return Err(VldError::single_with_value(IssueCode::NotInt, msg, value));
        }

        // Run other number checks
        self.inner.validate_number(n, value)?;

        Ok(n as i64)
    }
}
