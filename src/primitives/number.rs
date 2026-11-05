use serde_json::Value;
use std::cmp::Ordering;

use crate::error::{IssueCode, VldError};
use crate::schema::VldSchema;

const MAX_SAFE: i64 = 9_007_199_254_740_991;

/// A numeric value or bound: floats for [`ZNumber`], exact integers for [`ZInt`].
#[derive(Clone, Copy, Debug)]
enum Num {
    F(f64),
    I(i64),
}

impl Num {
    fn as_f64(self) -> f64 {
        match self {
            Num::F(f) => f,
            Num::I(i) => i as f64,
        }
    }

    /// `None` when either side is NaN — every bound check fails then.
    fn cmp(self, other: Num) -> Option<Ordering> {
        match (self, other) {
            (Num::I(a), Num::I(b)) => Some(a.cmp(&b)),
            (a, b) => a.as_f64().partial_cmp(&b.as_f64()),
        }
    }

    #[cfg(feature = "openapi")]
    fn json(self) -> Value {
        match self {
            Num::F(f) => serde_json::json!(f),
            Num::I(i) => serde_json::json!(i),
        }
    }
}

impl std::fmt::Display for Num {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Num::F(v) => write!(f, "{}", v),
            Num::I(v) => write!(f, "{}", v),
        }
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Min(Num),
    Max(Num),
    Gt(Num),
    Lt(Num),
    Positive,
    Negative,
    NonNegative,
    NonPositive,
    Finite,
    MultipleOf(Num),
    Safe,
}

#[derive(Clone)]
struct NumberCheck {
    kind: Kind,
    msg: String,
}

impl Kind {
    /// Stable key identifying the check category.
    fn key(self) -> &'static str {
        match self {
            Kind::Min(_) | Kind::Gt(_) => "too_small",
            Kind::Max(_) | Kind::Lt(_) => "too_big",
            Kind::Positive => "not_positive",
            Kind::Negative => "not_negative",
            Kind::NonNegative => "not_non_negative",
            Kind::NonPositive => "not_non_positive",
            Kind::Finite => "not_finite",
            Kind::MultipleOf(_) => "not_multiple_of",
            Kind::Safe => "not_safe",
        }
    }

    /// Lower bound `(value, exclusive)` this check implies, if any.
    fn lower(self) -> Option<(Num, bool)> {
        match self {
            Kind::Min(v) => Some((v, false)),
            Kind::Gt(v) => Some((v, true)),
            Kind::Positive => Some((Num::I(0), true)),
            Kind::NonNegative => Some((Num::I(0), false)),
            Kind::Safe => Some((Num::I(-MAX_SAFE), false)),
            _ => None,
        }
    }

    /// Upper bound `(value, exclusive)` this check implies, if any.
    fn upper(self) -> Option<(Num, bool)> {
        match self {
            Kind::Max(v) => Some((v, false)),
            Kind::Lt(v) => Some((v, true)),
            Kind::Negative => Some((Num::I(0), true)),
            Kind::NonPositive => Some((Num::I(0), false)),
            Kind::Safe => Some((Num::I(MAX_SAFE), false)),
            _ => None,
        }
    }

    /// `None` when `n` passes, otherwise the issue code.
    fn failure(self, n: Num) -> Option<IssueCode> {
        let custom = |code: &str| {
            Some(IssueCode::Custom {
                code: code.to_string(),
            })
        };
        match self {
            Kind::Finite => match n {
                Num::F(f) if !f.is_finite() => Some(IssueCode::NotFinite),
                _ => None,
            },
            Kind::MultipleOf(v) => {
                if is_multiple_of(n, v) {
                    None
                } else {
                    custom("not_multiple_of")
                }
            }
            Kind::Safe => {
                let ok = match n {
                    Num::I(i) => (-MAX_SAFE..=MAX_SAFE).contains(&i),
                    Num::F(f) => (-(MAX_SAFE as f64)..=MAX_SAFE as f64).contains(&f),
                };
                if ok {
                    None
                } else {
                    custom("not_safe")
                }
            }
            _ => {
                if let Some((b, excl)) = self.lower() {
                    let ok = matches!(
                        (n.cmp(b), excl),
                        (Some(Ordering::Greater), _) | (Some(Ordering::Equal), false)
                    );
                    if !ok {
                        return Some(IssueCode::TooSmall {
                            minimum: b.as_f64(),
                            inclusive: !excl,
                        });
                    }
                }
                if let Some((b, excl)) = self.upper() {
                    let ok = matches!(
                        (n.cmp(b), excl),
                        (Some(Ordering::Less), _) | (Some(Ordering::Equal), false)
                    );
                    if !ok {
                        return Some(IssueCode::TooBig {
                            maximum: b.as_f64(),
                            inclusive: !excl,
                        });
                    }
                }
                None
            }
        }
    }
}

/// `n` is a multiple of `v`. A zero divisor never matches; floats use a
/// relative tolerance so that e.g. `0.3` is a multiple of `0.1`.
fn is_multiple_of(n: Num, v: Num) -> bool {
    match (n, v) {
        (_, Num::I(0)) => false,
        (Num::I(a), Num::I(b)) => a % b == 0,
        (n, v) => {
            let (n, v) = (n.as_f64(), v.as_f64());
            if v == 0.0 || !n.is_finite() || !v.is_finite() {
                return false;
            }
            let q = n / v;
            (q - q.round()).abs() <= 1e-9 * q.abs().max(1.0)
        }
    }
}

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

    fn min_n(self, v: Num) -> Self {
        self.push(Kind::Min(v), format!("Number must be at least {}", v))
    }

    fn max_n(self, v: Num) -> Self {
        self.push(Kind::Max(v), format!("Number must be at most {}", v))
    }

    fn gt_n(self, v: Num) -> Self {
        self.push(Kind::Gt(v), format!("Number must be greater than {}", v))
    }

    fn lt_n(self, v: Num) -> Self {
        self.push(Kind::Lt(v), format!("Number must be less than {}", v))
    }

    fn multiple_of_n(self, v: Num) -> Self {
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
        ZInt {
            inner: self,
            custom_int_error: None,
        }
    }

    /// Coerce strings and booleans to numbers.
    ///
    /// Strings that parse to `NaN` or infinity are rejected.
    pub fn coerce(mut self) -> Self {
        self.coerce = true;
        self
    }

    fn type_err(&self, value: &Value) -> VldError {
        super::type_error(&self.custom_type_error, "number", "number", value)
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

    fn extract_number(&self, value: &Value) -> Result<f64, VldError> {
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

    fn validate(&self, n: Num, value: &Value) -> Result<(), VldError> {
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

// ---------------------------------------------------------------------------
// ZInt — integer validation (i64)
// ---------------------------------------------------------------------------

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
        let msg = self
            .custom_int_error
            .clone()
            .unwrap_or_else(|| "Expected integer, received float".to_string());
        VldError::single_with_value(IssueCode::NotInt, msg, value)
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
            minimum: i64::MIN as f64,
            inclusive: true,
        }
    } else {
        IssueCode::TooBig {
            maximum: i64::MAX as f64,
            inclusive: true,
        }
    };
    VldError::single_with_value(code, "Integer is out of the i64 range", value)
}

impl VldSchema for ZInt {
    type Output = i64;

    fn parse_value(&self, value: &Value) -> Result<i64, VldError> {
        let n = self.extract_int(value)?;
        self.inner.validate(Num::I(n), value)?;
        Ok(n)
    }
}
