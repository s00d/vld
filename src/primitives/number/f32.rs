use serde_json::Value;

use crate::error::{IssueCode, SizeOrigin, VldError};
use crate::schema::VldSchema;

use super::{Num, ZNumber};

/// Schema for `f32` numbers. Created via [`vld::f32()`](crate::f32).
///
/// Values outside the finite `f32` range fail with `too_small` / `too_big`
/// (origin `number`). Bounds and the input are compared at `f32` precision
/// (via their shortest decimal form), so `vld::f32().max(0.1)` accepts `0.1`,
/// `multiple_of(0.1)` accepts `0.3`, and messages print `0.1`.
///
/// JSON Schema: `{"type": "number", "format": "float"}` plus bounds.
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// let ratio: f32 = vld::f32().min(0.0).max(1.0).parse("0.25").unwrap();
/// assert_eq!(ratio, 0.25);
/// assert!(vld::f32().parse("1e39").is_err()); // above f32::MAX
/// ```
#[derive(Clone, Default)]
pub struct ZF32 {
    inner: ZNumber,
}

/// The `f64` nearest to the shortest decimal form of `v` (`0.1f32` → `0.1`,
/// not `0.10000000149011612`). Bounds and inputs are both widened this way,
/// so comparisons, `multiple_of` and messages work at `f32` precision.
fn widen(v: f32) -> f64 {
    v.to_string().parse().unwrap_or(f64::from(v))
}

macro_rules! bound_methods {
    ($($(#[$doc:meta])* $name:ident),+ $(,)?) => {$(
        $(#[$doc])*
        pub fn $name(mut self, val: f32) -> Self {
            self.inner = self.inner.$name(widen(val));
            self
        }
    )+};
}

macro_rules! flag_methods {
    ($($(#[$doc:meta])* $name:ident),+ $(,)?) => {$(
        $(#[$doc])*
        pub fn $name(mut self) -> Self {
            self.inner = self.inner.$name();
            self
        }
    )+};
}

impl ZF32 {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a custom error message for type mismatch (non-number input).
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.inner = self.inner.type_error(msg);
        self
    }

    /// Override error messages in bulk by check key (see [`ZNumber::with_messages`]).
    pub fn with_messages<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        self.inner = self.inner.with_messages(f);
        self
    }

    bound_methods! {
        /// Minimum value (inclusive).
        min,
        /// Maximum value (inclusive).
        max,
        /// Greater than (exclusive).
        gt,
        /// Greater than or equal (inclusive). Same as `min`.
        gte,
        /// Less than (exclusive).
        lt,
        /// Less than or equal (inclusive). Same as `max`.
        lte,
        /// Must be a multiple of the given value (`0` never matches).
        multiple_of,
    }

    flag_methods! {
        /// Must be positive (> 0).
        positive,
        /// Must be negative (< 0).
        negative,
        /// Must be non-negative (>= 0).
        non_negative,
        /// Must be non-positive (<= 0).
        non_positive,
        /// Coerce strings and booleans to numbers.
        coerce,
    }
}

impl VldSchema for ZF32 {
    type Output = f32;

    fn parse_value(&self, value: &Value) -> Result<f32, VldError> {
        let f = self.inner.extract_number(value)?;
        let out = f as f32;
        // Rounding to f32 overflows to infinity only past the finite range
        // (values within half an ulp of f32::MAX round down to it).
        let limit = widen(f32::MAX);
        if out == f32::NEG_INFINITY {
            return Err(VldError::from_code_with_value(
                IssueCode::TooSmall {
                    origin: SizeOrigin::Number,
                    minimum: (-limit).into(),
                    inclusive: true,
                },
                value,
            ));
        }
        if out == f32::INFINITY {
            return Err(VldError::from_code_with_value(
                IssueCode::TooBig {
                    origin: SizeOrigin::Number,
                    maximum: limit.into(),
                    inclusive: true,
                },
                value,
            ));
        }
        self.inner.validate(Num::F(widen(out)), value)?;
        Ok(out)
    }
}

#[cfg(feature = "openapi")]
impl crate::openapi::JsonSchema for ZF32 {
    fn json_schema(&self) -> serde_json::Value {
        let mut schema = self.inner.schema("number", None);
        schema["format"] = serde_json::json!("float");
        schema
    }
}

#[cfg(feature = "openapi")]
impl crate::openapi::CollectNestedSchemas for ZF32 {}
