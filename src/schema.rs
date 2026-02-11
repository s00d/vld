use serde_json::Value;
use std::marker::PhantomData;

use crate::combinators::{
    ZCatch, ZDescribe, ZIntersection, ZPipe, ZRefine, ZSuperRefine, ZTransform, ZUnion2,
};
use crate::error::VldError;
use crate::input::VldInput;
use crate::modifiers::{ZDefault, ZNullable, ZNullish, ZOptional};

/// Core validation schema trait.
///
/// Every validator in `vld` implements this trait. The associated type `Output`
/// defines what Rust type will be produced after successful parsing.
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// let schema = vld::string().min(3);
/// let result = schema.parse(r#""hello""#);
/// assert!(result.is_ok());
/// ```
pub trait VldSchema: Sized {
    /// The Rust type produced by this schema after successful parsing.
    type Output;

    /// Parse and validate a `serde_json::Value`.
    fn parse_value(&self, value: &Value) -> Result<Self::Output, VldError>;

    /// Parse from any supported input (JSON string, file path, `serde_json::Value`, etc.)
    fn parse<I: VldInput + ?Sized>(&self, input: &I) -> Result<Self::Output, VldError> {
        let json = input.to_json_value()?;
        self.parse_value(&json)
    }

    /// Validate an existing Rust value against this schema.
    ///
    /// The value is serialized to JSON via `serde`, then validated.
    /// Returns the parsed output on success.
    ///
    /// Requires the `serialize` feature.
    ///
    /// # Example
    /// ```ignore
    /// use vld::prelude::*;
    ///
    /// let schema = vld::array(vld::number().int().positive()).min_len(1);
    /// let data = vec![1, 2, 3];
    /// assert!(schema.validate(&data).is_ok());
    /// ```
    #[cfg(feature = "serialize")]
    fn validate<T: serde::Serialize>(&self, value: &T) -> Result<Self::Output, VldError> {
        let json = serde_json::to_value(value).map_err(|e| {
            VldError::single(
                crate::error::IssueCode::ParseError,
                format!("Serialization error: {}", e),
            )
        })?;
        self.parse_value(&json)
    }

    /// Check if an existing Rust value passes this schema's validation.
    ///
    /// Requires the `serialize` feature.
    ///
    /// # Example
    /// ```ignore
    /// use vld::prelude::*;
    ///
    /// let schema = vld::string().email();
    /// assert!(schema.is_valid(&"user@example.com"));
    /// ```
    #[cfg(feature = "serialize")]
    fn is_valid<T: serde::Serialize>(&self, value: &T) -> bool {
        self.validate(value).is_ok()
    }

    /// Make this field optional. Missing or null values become `None`.
    fn optional(self) -> ZOptional<Self> {
        ZOptional::new(self)
    }

    /// Allow null values. Null becomes `None`.
    fn nullable(self) -> ZNullable<Self> {
        ZNullable::new(self)
    }

    /// Provide a default value when the field is missing or null.
    fn with_default(self, value: Self::Output) -> ZDefault<Self>
    where
        Self::Output: Clone,
    {
        ZDefault::new(self, value)
    }

    /// Add a custom refinement check without changing the output type.
    fn refine<F>(self, check: F, message: &str) -> ZRefine<Self, F>
    where
        F: Fn(&Self::Output) -> bool,
    {
        ZRefine::new(self, check, message)
    }

    /// Transform the output value after successful parsing.
    fn transform<F, U>(self, f: F) -> ZTransform<Self, F, U>
    where
        F: Fn(Self::Output) -> U,
    {
        ZTransform::new(self, f)
    }

    /// Make this field nullish (both optional and nullable).
    fn nullish(self) -> ZNullish<Self> {
        ZNullish::new(self)
    }

    /// Return a fallback value on ANY validation error.
    fn catch(self, fallback: Self::Output) -> ZCatch<Self>
    where
        Self::Output: Clone,
    {
        ZCatch::new(self, fallback)
    }

    /// Chain this schema's output into another schema.
    ///
    /// The output of `self` is serialized to JSON, then parsed by `next`.
    fn pipe<S: VldSchema>(self, next: S) -> ZPipe<Self, S>
    where
        Self::Output: serde::Serialize,
    {
        ZPipe::new(self, next)
    }

    /// Attach a human-readable description/label to this schema.
    ///
    /// The description is stored as metadata and does not affect validation.
    fn describe(self, description: &str) -> ZDescribe<Self> {
        ZDescribe::new(self, description)
    }

    /// Add a custom refinement that can produce multiple errors.
    ///
    /// Unlike `refine()` which returns a single bool, `super_refine` receives
    /// a mutable `VldError` collector and can push multiple issues.
    fn super_refine<F>(self, check: F) -> ZSuperRefine<Self, F>
    where
        F: Fn(&Self::Output, &mut VldError),
    {
        ZSuperRefine::new(self, check)
    }

    /// Create a union: this schema **or** another. Returns `Either<Self::Output, B::Output>`.
    fn or<B: VldSchema>(self, other: B) -> ZUnion2<Self, B> {
        ZUnion2::new(self, other)
    }

    /// Create an intersection: input must satisfy **both** schemas.
    fn and<B: VldSchema>(self, other: B) -> ZIntersection<Self, B> {
        ZIntersection::new(self, other)
    }

    /// Override the error message for this schema.
    ///
    /// On validation failure **all** issues produced by the inner schema
    /// will have their message replaced with the provided string.
    ///
    /// Similar to Zod's `.message("...")`.
    ///
    /// # Example
    /// ```
    /// use vld::prelude::*;
    ///
    /// let schema = vld::string().min(3).message("Too short");
    /// let err = schema.parse(r#""ab""#).unwrap_err();
    /// assert_eq!(err.issues[0].message, "Too short");
    /// ```
    fn message(self, msg: impl Into<String>) -> crate::combinators::ZMessage<Self> {
        crate::combinators::ZMessage::new(self, msg)
    }
}

/// Trait for types that can be parsed from a `serde_json::Value`.
///
/// Auto-implemented by the [`schema!`](crate::schema!) macro and
/// [`#[derive(Validate)]`](crate::Validate) derive macro.
///
/// Used by framework integration crates (e.g., `vld-axum`, `vld-actix`)
/// to provide type-safe request body extraction.
pub trait VldParse: Sized {
    /// Parse and validate a `serde_json::Value` into this type.
    fn vld_parse_value(value: &serde_json::Value) -> Result<Self, crate::error::VldError>;
}

/// Schema for parsing nested structures. Created via [`vld::nested()`](crate::nested).
pub struct NestedSchema<T, F>
where
    F: Fn(&Value) -> Result<T, VldError>,
{
    parse_fn: F,
    _phantom: PhantomData<T>,
}

impl<T, F> NestedSchema<T, F>
where
    F: Fn(&Value) -> Result<T, VldError>,
{
    pub fn new(f: F) -> Self {
        Self {
            parse_fn: f,
            _phantom: PhantomData,
        }
    }
}

impl<T, F> VldSchema for NestedSchema<T, F>
where
    F: Fn(&Value) -> Result<T, VldError>,
{
    type Output = T;

    fn parse_value(&self, value: &Value) -> Result<T, VldError> {
        (self.parse_fn)(value)
    }
}
