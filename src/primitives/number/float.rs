use serde_json::Value;
#[cfg(feature = "openapi")]
use std::cmp::Ordering;

use crate::error::{issue_error, IssueCode, ValidationIssue, VldError};
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
    messages: crate::primitives::Messages,
}

impl ZNumber {
    pub fn new() -> Self {
        Self {
            checks: vec![],
            coerce: false,
            custom_type_error: None,
            messages: None,
        }
    }

    fn push(mut self, kind: Kind) -> Self {
        self.checks.push(NumberCheck { kind, msg: None });
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
    /// The closure is consulted when an issue is built, so it covers checks
    /// added before **and after** this call; a per-check message
    /// (`*_msg(...)`) takes precedence over it. A later call replaces it.
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
        F: Fn(&str) -> Option<String> + Send + Sync + 'static,
    {
        self.messages = Some(std::sync::Arc::new(f));
        self
    }

    pub(super) fn min_n(self, v: Num) -> Self {
        self.push(Kind::Min(v))
    }

    pub(super) fn max_n(self, v: Num) -> Self {
        self.push(Kind::Max(v))
    }

    pub(super) fn gt_n(self, v: Num) -> Self {
        self.push(Kind::Gt(v))
    }

    pub(super) fn lt_n(self, v: Num) -> Self {
        self.push(Kind::Lt(v))
    }

    pub(super) fn multiple_of_n(self, v: Num) -> Self {
        self.push(Kind::MultipleOf(v))
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
        self.push(Kind::Positive)
    }

    /// Must be negative (< 0).
    pub fn negative(self) -> Self {
        self.push(Kind::Negative)
    }

    /// Must be non-negative (>= 0).
    pub fn non_negative(self) -> Self {
        self.push(Kind::NonNegative)
    }

    /// Must be non-positive (<= 0).
    pub fn non_positive(self) -> Self {
        self.push(Kind::NonPositive)
    }

    /// Must be finite (not NaN or infinity).
    ///
    /// A no-op: JSON numbers are always finite and `.coerce()` already
    /// rejects `"NaN"` / `"inf"` strings.
    #[deprecated(
        since = "0.5.0",
        note = "no-op: JSON cannot carry NaN/infinity and coerce() rejects them"
    )]
    pub fn finite(self) -> Self {
        self.push(Kind::Finite)
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
        self.push(Kind::Safe)
    }

    /// Convert to integer validation. Returns `ZInt` with `Output = i64`.
    pub fn int(self) -> ZInt {
        ZInt::from_number(self)
    }

    /// Coerce strings and booleans to numbers.
    ///
    /// Strings that parse to `NaN` or infinity are rejected.
    pub fn coerce(mut self) -> Self {
        self.coerce = true;
        self
    }

    fn type_err(&self, value: &Value) -> VldError {
        crate::primitives::type_error(&self.custom_type_error, "number", value)
    }

    fn coerce_err(&self, value: &Value) -> VldError {
        issue_error(
            IssueCode::InvalidType {
                expected: "number".to_string(),
                received: "string".to_string(),
            },
            self.custom_type_error.as_ref(),
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
                .ok_or_else(|| self.coerce_err(value)),
            Value::Bool(b) if self.coerce => Ok(if *b { 1.0 } else { 0.0 }),
            _ => Err(self.type_err(value)),
        }
    }

    pub(super) fn validate(&self, n: Num, value: &Value) -> Result<(), VldError> {
        let mut errors = VldError::new();
        for check in &self.checks {
            if let Some(code) = check.kind.failure(n) {
                let bulk = || self.messages.as_ref().and_then(|f| f(check.kind.key()));
                let msg = check.msg.clone().or_else(bulk);
                errors.issues.push(
                    ValidationIssue::with_override(code, msg.as_ref()).with_received(value),
                );
            }
        }
        if errors.is_empty() {
            Ok(())
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

impl VldSchema for ZNumber {
    type Output = f64;

    fn parse_value(&self, value: &Value) -> Result<f64, VldError> {
        let n = self.extract_number(value)?;
        self.validate(Num::F(n), value)?;
        Ok(n)
    }
}

/// Generate a JSON Schema representation of this number schema.
///
/// The strictest lower/upper bound (from `min`/`gt`/`positive`/
/// `non_negative`/`safe`, …) is emitted as `minimum` or `exclusiveMinimum`
/// (likewise for the maximum). The first `multiple_of` goes into
/// `multipleOf`, further ones into `allOf`.
#[cfg(feature = "openapi")]
impl crate::openapi::JsonSchema for ZNumber {
    fn json_schema(&self) -> serde_json::Value {
        self.schema("number", None)
    }
}

impl ZNumber {
    /// JSON Schema of the checks; `range` is the output type's own
    /// `[min, max]`, intersected with the user bounds.
    #[cfg(feature = "openapi")]
    pub(super) fn schema(&self, ty: &str, range: Option<(Num, Num)>) -> Value {
        let mut schema = serde_json::json!({ "type": ty });
        // Strictest bound: larger lower bound wins, exclusive wins ties.
        let pick = |a: (Num, bool), b: (Num, bool), want: Ordering| match b.0.cmp(a.0) {
            Some(o) if o == want => b,
            Some(Ordering::Equal) if b.1 => b,
            _ => a,
        };
        let lower = range
            .map(|(lo, _)| (lo, false))
            .into_iter()
            .chain(self.checks.iter().filter_map(|c| c.kind.lower()))
            .reduce(|a, b| pick(a, b, Ordering::Greater));
        let upper = range
            .map(|(_, hi)| (hi, false))
            .into_iter()
            .chain(self.checks.iter().filter_map(|c| c.kind.upper()))
            .reduce(|a, b| pick(a, b, Ordering::Less));
        if let Some((v, excl)) = lower {
            let key = if excl { "exclusiveMinimum" } else { "minimum" };
            schema[key] = v.json();
        }
        if let Some((v, excl)) = upper {
            let key = if excl { "exclusiveMaximum" } else { "maximum" };
            schema[key] = v.json();
        }
        // `multipleOf` must be > 0: a zero divisor (never matches) is omitted.
        let mut multiples = self.checks.iter().filter_map(|c| match c.kind {
            Kind::MultipleOf(v) if v.as_f64() != 0.0 => Some(v.json()),
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

#[cfg(feature = "openapi")]
impl crate::openapi::CollectNestedSchemas for ZNumber {}
