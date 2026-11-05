use serde_json::Value;
use std::marker::PhantomData;

use crate::error::{issue_error, IssueCode, SizeOrigin, VldError};
use crate::schema::VldSchema;

use super::{Num, ZNumber};

mod sealed {
    pub trait Sealed {}
}

/// A Rust integer type usable with [`ZInteger`]: `i8`..`i64`, `u8`..`u64`,
/// `isize`, `usize`. Sealed.
pub trait IntType: sealed::Sealed + Copy + 'static {
    /// Smallest value of the type.
    const MIN: i128;
    /// Largest value of the type.
    const MAX: i128;
    /// OpenAPI `format` (`int8`, …, `uint64`).
    const FORMAT: &'static str;
    #[doc(hidden)]
    fn from_i128(v: i128) -> Self;
    #[doc(hidden)]
    fn to_i128(self) -> i128;
}

macro_rules! int_types {
    ($($t:ty => $fmt:expr),+ $(,)?) => {$(
        impl sealed::Sealed for $t {}
        impl IntType for $t {
            const MIN: i128 = <$t>::MIN as i128;
            const MAX: i128 = <$t>::MAX as i128;
            const FORMAT: &'static str = $fmt;
            fn from_i128(v: i128) -> Self {
                v as $t
            }
            fn to_i128(self) -> i128 {
                self as i128
            }
        }
    )+};
}

int_types! {
    i8 => "int8", i16 => "int16", i32 => "int32", i64 => "int64",
    u8 => "uint8", u16 => "uint16", u32 => "uint32", u64 => "uint64",
    isize => if isize::BITS == 64 { "int64" } else { "int32" },
    usize => if usize::BITS == 64 { "uint64" } else { "uint32" },
}

/// Schema for a sized integer (`i8`..`u64`, `isize`, `usize`). Created via
/// [`vld::i8()`](crate::i8) … [`vld::u64()`](crate::u64); [`ZInt`]
/// (`vld::number().int()`) is `ZInteger<i64>`.
///
/// Integers are read exactly (no `f64` round-trip; `u64` values above
/// `i64::MAX` included). Floats with a fraction fail with `not_int`; values
/// outside the type's range fail with `too_small` / `too_big` (origin
/// `number`) naming the type's bound.
///
/// JSON Schema: `{"type": "integer", "format": "int8" … "uint64"}` with
/// `minimum` / `maximum` from the type range intersected with the user bounds.
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// let port: u16 = vld::u16().min(1).parse("8080").unwrap();
/// assert_eq!(port, 8080);
/// assert!(vld::u16().parse("70000").is_err()); // above u16::MAX
/// assert!(vld::u8().parse("-1").is_err());
/// assert!(vld::i32().parse("1.5").is_err());
/// assert_eq!(vld::u64().parse("18446744073709551615").unwrap(), u64::MAX);
/// ```
pub struct ZInteger<T: IntType> {
    inner: ZNumber,
    custom_int_error: Option<String>,
    /// `false` for `number().int()`: JSON Schema without `format` / type range.
    typed: bool,
    _type: PhantomData<fn() -> T>,
}

/// Integer schema with `i64` output. Created via `vld::number().int()`.
///
/// Same as `ZInteger<i64>` except that its JSON Schema carries no `format`
/// and no `i64` range (use [`vld::i64()`](crate::i64) for those).
pub type ZInt = ZInteger<i64>;

impl<T: IntType> Clone for ZInteger<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            custom_int_error: self.custom_int_error.clone(),
            typed: self.typed,
            _type: PhantomData,
        }
    }
}

impl<T: IntType> Default for ZInteger<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl ZInteger<i64> {
    pub(super) fn from_number(inner: ZNumber) -> Self {
        Self {
            inner,
            custom_int_error: None,
            typed: false,
            _type: PhantomData,
        }
    }
}

impl<T: IntType> ZInteger<T> {
    pub fn new() -> Self {
        Self {
            inner: ZNumber::new(),
            custom_int_error: None,
            typed: true,
            _type: PhantomData,
        }
    }

    fn with(mut self, f: impl FnOnce(ZNumber) -> ZNumber) -> Self {
        self.inner = f(self.inner);
        self
    }

    /// Set a custom error message for type mismatch (non-number input).
    pub fn type_error(self, msg: impl Into<String>) -> Self {
        self.with(|n| n.type_error(msg))
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
        self.with(|n| n.with_messages(f))
    }

    /// Coerce strings (`"42"`) and booleans to integers.
    pub fn coerce(mut self) -> Self {
        self.inner.coerce = true;
        self
    }

    /// Minimum value (inclusive).
    pub fn min(self, val: T) -> Self {
        self.with(|n| n.min_n(Num::I(val.to_i128())))
    }

    /// Maximum value (inclusive).
    pub fn max(self, val: T) -> Self {
        self.with(|n| n.max_n(Num::I(val.to_i128())))
    }

    /// Greater than (exclusive).
    pub fn gt(self, val: T) -> Self {
        self.with(|n| n.gt_n(Num::I(val.to_i128())))
    }

    /// Greater than or equal (inclusive). Same as `min`.
    pub fn gte(self, val: T) -> Self {
        self.min(val)
    }

    /// Less than (exclusive).
    pub fn lt(self, val: T) -> Self {
        self.with(|n| n.lt_n(Num::I(val.to_i128())))
    }

    /// Less than or equal (inclusive). Same as `max`.
    pub fn lte(self, val: T) -> Self {
        self.max(val)
    }

    /// Must be positive (> 0).
    pub fn positive(self) -> Self {
        self.with(ZNumber::positive)
    }

    /// Must be negative (< 0).
    pub fn negative(self) -> Self {
        self.with(ZNumber::negative)
    }

    /// Must be non-negative (>= 0).
    pub fn non_negative(self) -> Self {
        self.with(ZNumber::non_negative)
    }

    /// Must be non-positive (<= 0).
    pub fn non_positive(self) -> Self {
        self.with(ZNumber::non_positive)
    }

    /// Must be within JavaScript's safe integer range.
    pub fn safe(self) -> Self {
        self.with(ZNumber::safe)
    }

    /// Must be a multiple of the given value (`0` never matches).
    pub fn multiple_of(self, val: T) -> Self {
        self.with(|n| n.multiple_of_n(Num::I(val.to_i128())))
    }

    fn not_int(&self, value: &Value) -> VldError {
        issue_error(IssueCode::NotInt, self.custom_int_error.as_ref(), value)
    }

    fn int_from_f64(&self, f: f64, value: &Value) -> Result<i128, VldError> {
        if f.fract() != 0.0 {
            return Err(self.not_int(value));
        }
        // Saturating cast; the range check rejects anything clamped.
        Ok(f as i128)
    }

    fn extract_int(&self, value: &Value) -> Result<i128, VldError> {
        let i = match value {
            Value::Number(n) => match (n.as_i64(), n.as_u64()) {
                (Some(i), _) => i128::from(i),
                (_, Some(u)) => i128::from(u),
                _ => self.int_from_f64(n.as_f64().unwrap_or(f64::NAN), value)?,
            },
            Value::String(s) if self.inner.coerce => match s.parse::<i128>() {
                Ok(i) => i,
                Err(_) => {
                    let f = self.inner.extract_number(value)?;
                    self.int_from_f64(f, value)?
                }
            },
            _ => self.inner.extract_number(value)? as i128,
        };
        if i < T::MIN {
            return Err(VldError::from_code_with_value(
                IssueCode::TooSmall {
                    origin: SizeOrigin::Number,
                    minimum: Num::I(T::MIN).json(),
                    inclusive: true,
                },
                value,
            ));
        }
        if i > T::MAX {
            return Err(VldError::from_code_with_value(
                IssueCode::TooBig {
                    origin: SizeOrigin::Number,
                    maximum: Num::I(T::MAX).json(),
                    inclusive: true,
                },
                value,
            ));
        }
        Ok(i)
    }
}

impl<T: IntType> VldSchema for ZInteger<T> {
    type Output = T;

    fn parse_value(&self, value: &Value) -> Result<T, VldError> {
        let n = self.extract_int(value)?;
        self.inner.validate(Num::I(n), value)?;
        Ok(T::from_i128(n))
    }
}

/// `{"type": "integer"}` plus bounds; typed integers (`vld::u8()`, …) add
/// `format` and the type range.
#[cfg(feature = "openapi")]
impl<T: IntType> crate::openapi::JsonSchema for ZInteger<T> {
    fn json_schema(&self) -> serde_json::Value {
        if !self.typed {
            return self.inner.schema("integer", None);
        }
        let mut schema = self
            .inner
            .schema("integer", Some((Num::I(T::MIN), Num::I(T::MAX))));
        schema["format"] = serde_json::json!(T::FORMAT);
        schema
    }
}

#[cfg(feature = "openapi")]
impl<T: IntType> crate::openapi::CollectNestedSchemas for ZInteger<T> {}
