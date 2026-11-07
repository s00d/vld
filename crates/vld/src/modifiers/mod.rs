mod default_val;
#[cfg(feature = "openapi")]
mod json_schema;

pub use default_val::ZDefault;

use serde_json::Value;

use crate::error::VldError;
use crate::schema::VldSchema;

/// `ZOptional`, `ZNullable` and `ZNullish` all parse `null` as `None` and
/// anything else through the inner schema. They differ for object fields and
/// in JSON Schema:
///
/// | | absent key | `null` | JSON Schema | TypeScript |
/// |---|---|---|---|---|
/// | `optional()` | `None` | `None` | not required | `x?: T` |
/// | `nullable()` | `missing_field` | `None` | required, `T \| null` | `x: T \| null` |
/// | `nullish()` | `None` | `None` | not required, `T \| null` | `x?: T \| null` |
///
/// `optional()` accepts `null` too because serde writes `None` as `null`, so
/// `validate()` of a struct with a `None` field must pass.
macro_rules! option_wrapper {
    ($(#[$doc:meta])* $name:ident $(, missing = $missing:expr)?) => {
        $(#[$doc])*
        pub struct $name<T: VldSchema> {
            inner: T,
        }

        impl<T: VldSchema> $name<T> {
            /// Create the schema (the same as its `vld::` constructor function).
            pub fn new(inner: T) -> Self {
                Self { inner }
            }

            /// Access the inner schema (for JSON Schema generation).
            pub fn inner_schema(&self) -> &T {
                &self.inner
            }
        }

        impl<T: VldSchema> VldSchema for $name<T> {
            type Output = Option<T::Output>;

            fn parse_value(&self, value: &Value) -> Result<Option<T::Output>, VldError> {
                if value.is_null() {
                    return Ok(None);
                }
                self.inner.parse_value(value).map(Some)
            }

            fn output_json(&self, o: &Self::Output) -> Option<Value> {
                match o {
                    Some(v) => self.inner.output_json(v),
                    None => Some(Value::Null),
                }
            }

            $(
                fn parse_missing(&self) -> Result<Option<T::Output>, VldError> {
                    $missing
                }
            )?
        }
    };
}

option_wrapper! {
    /// Wraps a schema to make it optional.
    ///
    /// If the value is `null` or missing, returns `Ok(None)`.
    /// Otherwise, delegates to the inner schema and wraps the result in `Some`.
    ZOptional
}

option_wrapper! {
    /// Wraps a schema to allow `null`.
    ///
    /// If the value is `null`, returns `Ok(None)`; otherwise delegates to the
    /// inner schema and wraps the result in `Some`.
    ///
    /// As an object field the key is **required**: an absent key fails with
    /// `missing_field` (use [`nullish()`](ZNullish) to allow both).
    ZNullable, missing = Err(VldError::from_code(crate::error::IssueCode::MissingField))
}

option_wrapper! {
    /// Wraps a schema to make it both optional and nullable (nullish).
    ///
    /// Equivalent to `.optional()` + `.nullable()` in Zod.
    /// If the value is `null` or missing, returns `Ok(None)`.
    ZNullish
}
