mod default_val;

pub use default_val::ZDefault;

use serde_json::Value;

use crate::error::VldError;
use crate::schema::VldSchema;

/// `ZOptional`, `ZNullable` and `ZNullish` behave identically at the value
/// level (`null` → `None`, otherwise `Some(inner)`); they stay distinct types
/// only for JSON Schema / object-level semantics.
macro_rules! option_wrapper {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        pub struct $name<T: VldSchema> {
            inner: T,
        }

        impl<T: VldSchema> $name<T> {
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
    /// A **missing** object field is treated like `null` (object parsing feeds
    /// `null` for absent keys), so it also yields `Ok(None)` — at the value
    /// level `nullable()` and [`optional()`](ZOptional) behave the same. The
    /// difference is only in the generated JSON Schema (`nullable` emits a
    /// `null`-able type rather than an optional property).
    ZNullable
}

option_wrapper! {
    /// Wraps a schema to make it both optional and nullable (nullish).
    ///
    /// Equivalent to `.optional()` + `.nullable()` in Zod.
    /// If the value is `null` or missing, returns `Ok(None)`.
    ZNullish
}
