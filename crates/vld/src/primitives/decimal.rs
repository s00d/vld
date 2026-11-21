use rust_decimal::Decimal;
use serde_json::Value;
use std::str::FromStr;

use crate::error::{FormatKind, IssueCode, SizeOrigin, VldError};
use crate::schema::VldSchema;

fn parse_literal(value: &str) -> Decimal {
    parse_decimal(value).unwrap_or_else(|| panic!("Invalid decimal literal: {}", value))
}

/// Plain (`1.5`) or scientific (`1e-7`, `1.5E30`) notation.
fn parse_decimal(s: &str) -> Option<Decimal> {
    Decimal::from_str(s)
        .or_else(|_| Decimal::from_scientific(s))
        .ok()
}

/// Schema for arbitrary-precision decimals (`rust_decimal::Decimal`), from a
/// JSON string or number. Created via [`vld::decimal()`](crate::decimal).
#[derive(Clone)]
pub struct ZDecimal {
    min: Option<Decimal>,
    max: Option<Decimal>,
    gt: Option<Decimal>,
    lt: Option<Decimal>,
    multiple_of: Option<Decimal>,
    positive: bool,
    negative: bool,
    non_negative: bool,
    non_positive: bool,
    max_scale: Option<u32>,
    scale: Option<u32>,
    max_precision: Option<u32>,
    custom_type_error: Option<String>,
    messages: crate::primitives::Messages,
}

impl ZDecimal {
    /// Create the schema (the same as its `vld::` constructor function).
    pub fn new() -> Self {
        Self {
            min: None,
            max: None,
            gt: None,
            lt: None,
            multiple_of: None,
            positive: false,
            negative: false,
            non_negative: false,
            non_positive: false,
            max_scale: None,
            scale: None,
            max_precision: None,
            custom_type_error: None,
            messages: None,
        }
    }

    /// Message used when the input is not a number or numeric string.
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    /// Minimum value (inclusive). Panics on an invalid literal.
    pub fn min(mut self, value: impl AsRef<str>) -> Self {
        self.min = Some(parse_literal(value.as_ref()));
        self
    }

    /// Maximum value (inclusive). Panics on an invalid literal.
    pub fn max(mut self, value: impl AsRef<str>) -> Self {
        self.max = Some(parse_literal(value.as_ref()));
        self
    }

    /// Greater than (exclusive). Panics on an invalid literal.
    pub fn gt(mut self, value: impl AsRef<str>) -> Self {
        self.gt = Some(parse_literal(value.as_ref()));
        self
    }

    /// Less than (exclusive). Panics on an invalid literal.
    pub fn lt(mut self, value: impl AsRef<str>) -> Self {
        self.lt = Some(parse_literal(value.as_ref()));
        self
    }

    /// Must be an exact multiple of `value` (`0` never matches).
    /// Panics on an invalid literal.
    pub fn multiple_of(mut self, value: impl AsRef<str>) -> Self {
        self.multiple_of = Some(parse_literal(value.as_ref()));
        self
    }

    /// Must be greater than zero.
    pub fn positive(mut self) -> Self {
        self.positive = true;
        self
    }

    /// Must be less than zero.
    pub fn negative(mut self) -> Self {
        self.negative = true;
        self
    }

    /// Must be zero or greater.
    pub fn non_negative(mut self) -> Self {
        self.non_negative = true;
        self
    }

    /// Must be zero or less.
    pub fn non_positive(mut self) -> Self {
        self.non_positive = true;
        self
    }

    /// At most `n` digits after the decimal point, **as written**: `"1.230"`
    /// has scale 3 (trailing zeros count — `"1.230"` fails `max_scale(2)`),
    /// `"1e-7"` has scale 7. A JSON number has the scale of its shortest
    /// form (`1.10` arrives as `1.1`). Fails with `too_big` (origin `scale`).
    pub fn max_scale(mut self, n: u32) -> Self {
        self.max_scale = Some(n);
        self
    }

    /// Exactly `n` digits after the decimal point, as written (`"1.50"` for
    /// `scale(2)`; `"1.5"` fails). Meant for decimal strings: JSON numbers
    /// lose trailing zeros. Fails with `invalid_length` (origin `scale`).
    pub fn scale(mut self, n: u32) -> Self {
        self.scale = Some(n);
        self
    }

    /// At most `p` significant digits in total, as written: the digits of
    /// the unscaled value, and at least the scale (`"123.45"` → 5,
    /// `"0.012"` → 3, `"1.230"` → 4, `"0"` → 1). Fails with `too_big`
    /// (origin `precision`).
    ///
    /// # Panics
    /// If `p` is 0 (no value has precision 0).
    pub fn max_precision(mut self, p: u32) -> Self {
        assert!(p > 0, "max_precision must be at least 1");
        self.max_precision = Some(p);
        self
    }

    /// Override messages in bulk by check key: `"invalid_format"`,
    /// `"too_small"` / `"too_big"` (bounds and sign checks),
    /// `"not_multiple_of"`, `"max_scale"`, `"scale"`, `"max_precision"`.
    /// [`type_error`](Self::type_error) keeps the type mismatch message.
    pub fn with_messages<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> Option<String> + Send + Sync + 'static,
    {
        self.messages = Some(std::sync::Arc::new(f));
        self
    }

    /// The error for check `key`, with its `with_messages` override.
    pub(crate) fn fail(&self, key: &str, code: IssueCode, value: &Value) -> VldError {
        let msg = self.messages.as_ref().and_then(|f| f(key));
        crate::error::issue_error(code, msg.as_ref(), value)
    }

    /// Scale / precision checks on a parsed value (also used by `money()`).
    pub(crate) fn check_digits(&self, n: &Decimal, value: &Value) -> Result<(), VldError> {
        if let Some(max) = self.max_scale.filter(|m| n.scale() > *m) {
            return Err(self.fail("max_scale", scale_too_big(max), value));
        }
        if let Some(want) = self.scale.filter(|s| n.scale() != *s) {
            let code = IssueCode::InvalidLength {
                origin: SizeOrigin::Scale,
                expected: want as usize,
            };
            return Err(self.fail("scale", code, value));
        }
        if let Some(max) = self.max_precision.filter(|m| precision(n) > *m) {
            let code = IssueCode::TooBig {
                origin: SizeOrigin::Precision,
                maximum: max.into(),
                inclusive: true,
            };
            return Err(self.fail("max_precision", code, value));
        }
        Ok(())
    }

    /// Strictest lower bound `(value, exclusive)` from all constraints.
    #[cfg(feature = "openapi")]
    fn lower(&self) -> Option<(Decimal, bool)> {
        [
            self.min.map(|v| (v, false)),
            self.gt.map(|v| (v, true)),
            self.positive.then_some((Decimal::ZERO, true)),
            self.non_negative.then_some((Decimal::ZERO, false)),
        ]
        .into_iter()
        .flatten()
        .reduce(|a, b| {
            if b.0 > a.0 || (b.0 == a.0 && b.1) {
                b
            } else {
                a
            }
        })
    }

    /// Strictest upper bound `(value, exclusive)` from all constraints.
    #[cfg(feature = "openapi")]
    fn upper(&self) -> Option<(Decimal, bool)> {
        [
            self.max.map(|v| (v, false)),
            self.lt.map(|v| (v, true)),
            self.negative.then_some((Decimal::ZERO, true)),
            self.non_positive.then_some((Decimal::ZERO, false)),
        ]
        .into_iter()
        .flatten()
        .reduce(|a, b| {
            if b.0 < a.0 || (b.0 == a.0 && b.1) {
                b
            } else {
                a
            }
        })
    }
}

impl Default for ZDecimal {
    fn default() -> Self {
        Self::new()
    }
}

fn too_small(bound: Decimal, inclusive: bool) -> IssueCode {
    IssueCode::TooSmall {
        origin: SizeOrigin::Decimal,
        minimum: Value::String(bound.to_string()),
        inclusive,
    }
}

fn too_big(bound: Decimal, inclusive: bool) -> IssueCode {
    IssueCode::TooBig {
        origin: SizeOrigin::Decimal,
        maximum: Value::String(bound.to_string()),
        inclusive,
    }
}

/// `too_big` with origin `scale`: more than `max` fraction digits.
pub(crate) fn scale_too_big(max: u32) -> IssueCode {
    IssueCode::TooBig {
        origin: SizeOrigin::Scale,
        maximum: max.into(),
        inclusive: true,
    }
}

/// Significant digits as written: digits of the unscaled value, at least the scale.
fn precision(n: &Decimal) -> u32 {
    let digits = n
        .mantissa()
        .unsigned_abs()
        .checked_ilog10()
        .map_or(1, |d| d + 1);
    digits.max(n.scale())
}

impl VldSchema for ZDecimal {
    type Output = Decimal;

    fn parse_value(&self, value: &Value) -> Result<Decimal, VldError> {
        let s = match value {
            Value::String(s) => s.clone(),
            // serde_json prints some floats in exponent form (`1e-7`, `1e30`).
            Value::Number(n) => n.to_string(),
            _ => {
                return Err(crate::primitives::type_error(
                    &self.custom_type_error,
                    "decimal",
                    value,
                ));
            }
        };
        let n = parse_decimal(&s).ok_or_else(|| {
            let code = IssueCode::InvalidFormat {
                format: FormatKind::Decimal,
            };
            self.fail("invalid_format", code, value)
        })?;

        let zero = Decimal::ZERO;
        let failure = [
            self.min.filter(|m| n < *m).map(|m| too_small(m, true)),
            self.max.filter(|m| n > *m).map(|m| too_big(m, true)),
            self.gt.filter(|g| n <= *g).map(|g| too_small(g, false)),
            self.lt.filter(|l| n >= *l).map(|l| too_big(l, false)),
            self.multiple_of
                .filter(|m| m.is_zero() || !n.checked_rem(*m).is_some_and(|r| r.is_zero()))
                .map(|m| IssueCode::NotMultipleOf {
                    divisor: Value::String(m.to_string()),
                }),
            (self.positive && n <= zero).then(|| too_small(zero, false)),
            (self.negative && n >= zero).then(|| too_big(zero, false)),
            (self.non_negative && n < zero).then(|| too_small(zero, true)),
            (self.non_positive && n > zero).then(|| too_big(zero, true)),
        ]
        .into_iter()
        .flatten()
        .next();
        if let Some(code) = failure {
            let key = code.key().to_string();
            return Err(self.fail(&key, code, value));
        }
        self.check_digits(&n, value)?;
        Ok(n)
    }
}

/// JSON Schema: a `decimal`-format string with `x-minDecimal` /
/// `x-exclusiveMinDecimal` (strictest lower bound, sign flags included),
/// `x-maxDecimal` / `x-exclusiveMaxDecimal`, `x-multipleOfDecimal`,
/// `x-maxScale`, `x-scale` and `x-maxPrecision` (vld extensions: standard
/// validators ignore them, `vld::fake` honours them).
#[cfg(feature = "openapi")]
impl crate::openapi::JsonSchema for ZDecimal {
    fn json_schema(&self) -> serde_json::Value {
        // JSON numbers are accepted as well as decimal strings.
        let mut schema = serde_json::json!({
            "type": ["string", "number"],
            "format": "decimal"
        });
        if let Some((v, excl)) = self.lower() {
            let key = if excl {
                "x-exclusiveMinDecimal"
            } else {
                "x-minDecimal"
            };
            schema[key] = serde_json::json!(v.to_string());
        }
        if let Some((v, excl)) = self.upper() {
            let key = if excl {
                "x-exclusiveMaxDecimal"
            } else {
                "x-maxDecimal"
            };
            schema[key] = serde_json::json!(v.to_string());
        }
        if let Some(m) = self.multiple_of {
            schema["x-multipleOfDecimal"] = serde_json::json!(m.to_string());
        }
        for (key, v) in [
            ("x-maxScale", self.max_scale),
            ("x-scale", self.scale),
            ("x-maxPrecision", self.max_precision),
        ] {
            if let Some(v) = v {
                schema[key] = v.into();
            }
        }
        schema
    }
}

#[cfg(feature = "openapi")]
impl crate::openapi::CollectNestedSchemas for ZDecimal {}
