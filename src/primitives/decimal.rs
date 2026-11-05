use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json::Value;
use std::str::FromStr;

use crate::error::{IssueCode, VldError};
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
    custom_type_error: Option<String>,
}

impl ZDecimal {
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
            custom_type_error: None,
        }
    }

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

    pub fn positive(mut self) -> Self {
        self.positive = true;
        self
    }

    pub fn negative(mut self) -> Self {
        self.negative = true;
        self
    }

    pub fn non_negative(mut self) -> Self {
        self.non_negative = true;
        self
    }

    pub fn non_positive(mut self) -> Self {
        self.non_positive = true;
        self
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

    /// JSON Schema: a `decimal`-format string with `x-minDecimal` /
    /// `x-exclusiveMinDecimal` (strictest lower bound, sign flags included),
    /// `x-maxDecimal` / `x-exclusiveMaxDecimal` and `x-multipleOfDecimal`.
    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        let mut schema = serde_json::json!({
            "type": "string",
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
        schema
    }
}

impl Default for ZDecimal {
    fn default() -> Self {
        Self::new()
    }
}

fn too_small(bound: Decimal, inclusive: bool, msg: String, value: &Value) -> VldError {
    VldError::single_with_value(
        IssueCode::TooSmall {
            minimum: bound.to_f64().unwrap_or(0.0),
            inclusive,
        },
        msg,
        value,
    )
}

fn too_big(bound: Decimal, inclusive: bool, msg: String, value: &Value) -> VldError {
    VldError::single_with_value(
        IssueCode::TooBig {
            maximum: bound.to_f64().unwrap_or(0.0),
            inclusive,
        },
        msg,
        value,
    )
}

fn custom(code: &str, msg: &str, value: &Value) -> VldError {
    VldError::single_with_value(
        IssueCode::Custom {
            code: code.to_string(),
        },
        msg,
        value,
    )
}

impl VldSchema for ZDecimal {
    type Output = Decimal;

    fn parse_value(&self, value: &Value) -> Result<Decimal, VldError> {
        let s = match value {
            Value::String(s) => s.clone(),
            // serde_json prints some floats in exponent form (`1e-7`, `1e30`).
            Value::Number(n) => n.to_string(),
            _ => {
                return Err(super::type_error(
                    &self.custom_type_error,
                    "decimal",
                    "decimal as string or number",
                    value,
                ))
            }
        };
        let n = parse_decimal(&s).ok_or_else(|| {
            custom(
                "invalid_decimal",
                &format!("Invalid decimal value: {}", s),
                value,
            )
        })?;

        if let Some(min) = self.min.filter(|m| n < *m) {
            return Err(too_small(
                min,
                true,
                format!("Decimal must be at least {}", min),
                value,
            ));
        }
        if let Some(max) = self.max.filter(|m| n > *m) {
            return Err(too_big(
                max,
                true,
                format!("Decimal must be at most {}", max),
                value,
            ));
        }
        if let Some(gt) = self.gt.filter(|g| n <= *g) {
            return Err(too_small(
                gt,
                false,
                format!("Decimal must be greater than {}", gt),
                value,
            ));
        }
        if let Some(lt) = self.lt.filter(|l| n >= *l) {
            return Err(too_big(
                lt,
                false,
                format!("Decimal must be less than {}", lt),
                value,
            ));
        }
        if let Some(m) = self.multiple_of {
            if m.is_zero() || !n.checked_rem(m).is_some_and(|r| r.is_zero()) {
                return Err(custom(
                    "not_multiple_of",
                    &format!("Decimal must be a multiple of {}", m),
                    value,
                ));
            }
        }
        let sign_checks = [
            (
                self.positive && n <= Decimal::ZERO,
                "not_positive",
                "Decimal must be positive",
            ),
            (
                self.negative && n >= Decimal::ZERO,
                "not_negative",
                "Decimal must be negative",
            ),
            (
                self.non_negative && n < Decimal::ZERO,
                "not_non_negative",
                "Decimal must be non-negative",
            ),
            (
                self.non_positive && n > Decimal::ZERO,
                "not_non_positive",
                "Decimal must be non-positive",
            ),
        ];
        if let Some((_, code, msg)) = sign_checks.into_iter().find(|(failed, ..)| *failed) {
            return Err(custom(code, msg, value));
        }
        Ok(n)
    }
}
