use serde_json::Value;

use crate::combinators::{
    ZCatch, ZDescribe, ZIntersection, ZPipe, ZRefine, ZSuperRefine, ZTransform, ZUnion2,
};
use crate::error::VldError;
use crate::modifiers::{ZDefault, ZNullable, ZNullish, ZOptional};

mod coerced;
mod dyn_schema;
/// Input sources accepted by `parse` (JSON text, files, values).
pub mod input;
mod nested;

pub(crate) use dyn_schema::to_json_value;

pub use coerced::parse_coerced;
pub use dyn_schema::DynSchema;
pub use input::VldInput;
pub use nested::{NestedSchema, NestedSchemaCollectFn, NestedSchemaEntry};

/// A schema with everything the enabled features need: [`VldSchema`], plus
/// [`JsonSchema`](crate::openapi::JsonSchema) and
/// [`CollectNestedSchemas`](crate::openapi::CollectNestedSchemas) with the
/// `openapi` feature.
///
/// Use it to return a schema from a function without naming its type: an
/// `impl VldSchema` hides the JSON Schema impls, so the result could not be
/// used in an object or `schema!` field with `openapi` on.
///
/// ```
/// use vld::prelude::*;
///
/// fn username() -> impl vld::Schema<Output = String> {
///     vld::string().min(3).max(20).alphanumeric()
/// }
///
/// let user = vld::object().field("name", username());
/// assert!(user.parse(r#"{"name": "ada"}"#).is_ok());
/// ```
#[cfg(feature = "openapi")]
pub trait Schema:
    VldSchema + crate::openapi::JsonSchema + crate::openapi::CollectNestedSchemas
{
}

#[cfg(feature = "openapi")]
impl<T> Schema for T where
    T: VldSchema + crate::openapi::JsonSchema + crate::openapi::CollectNestedSchemas
{
}

/// A schema with everything the enabled features need (here: [`VldSchema`];
/// with `openapi` also the JSON Schema traits).
#[cfg(not(feature = "openapi"))]
pub trait Schema: VldSchema {}

#[cfg(not(feature = "openapi"))]
impl<T: VldSchema> Schema for T {}

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

    /// The JSON form of an output value, when it differs from its `serde`
    /// form (e.g. `time` datetimes serialize as non-RFC 3339 text). Used for
    /// the JSON Schema `default` of `.with_default(...)`; `None` means use
    /// `serde_json::to_value`.
    #[doc(hidden)]
    fn output_json(&self, _output: &Self::Output) -> Option<Value> {
        None
    }

    /// Parse an object field whose key is **absent**.
    ///
    /// Default: the same as an explicit `null` (`parse_value(&Value::Null)`),
    /// so an absent key and `null` are treated alike. `nullable()` overrides
    /// it to require the key (`missing_field`); wrappers (`describe`,
    /// `refine`, `transform`, `rule`, …) forward it to their inner schema.
    fn parse_missing(&self) -> Result<Self::Output, VldError> {
        self.parse_value(&Value::Null)
    }

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
    /// # Example
    /// ```
    /// use vld::prelude::*;
    ///
    /// let schema = vld::array(vld::number().int().positive()).min_len(1);
    /// let data = vec![1, 2, 3];
    /// assert!(schema.validate(&data).is_ok());
    /// ```
    fn validate<T: serde::Serialize>(&self, value: &T) -> Result<Self::Output, VldError> {
        self.parse_value(&to_json_value(value)?)
    }

    /// Check if an existing Rust value passes this schema's validation.
    ///
    /// # Example
    /// ```
    /// use vld::prelude::*;
    ///
    /// let schema = vld::string().email();
    /// assert!(schema.is_valid(&"user@example.com"));
    /// ```
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

    /// Check the output with a reusable [`Rule`](crate::rules::Rule) (output type unchanged).
    ///
    /// ```
    /// use vld::prelude::*;
    /// let even = vld::number().int().rule(vld::rules::from_fn("even", |n: &i64| n % 2 == 0));
    /// assert_eq!(even.parse("3").unwrap_err().issues[0].code.key(), "even");
    /// ```
    fn rule<R>(self, rule: R) -> crate::rules::ZRule<Self, R>
    where
        Self: Sized,
        R: crate::rules::Rule<Self::Output>,
    {
        crate::rules::ZRule::new(self, rule, None)
    }

    /// [`rule`](Self::rule) with a custom error message.
    fn rule_msg<R>(self, rule: R, msg: impl Into<String>) -> crate::rules::ZRule<Self, R>
    where
        Self: Sized,
        R: crate::rules::Rule<Self::Output>,
    {
        crate::rules::ZRule::new(self, rule, Some(msg.into()))
    }

    /// Transform the output with a fallible function; an `Err` becomes a
    /// validation issue (`Custom { code: "transform" }`, message = the error).
    ///
    /// ```
    /// use vld::prelude::*;
    /// let big = vld::string().try_transform(|s| s.parse::<u128>());
    /// assert_eq!(big.parse(r#""340282366920938463463374607431768211455""#).unwrap(), u128::MAX);
    /// assert_eq!(big.parse(r#""x""#).unwrap_err().issues[0].code.key(), "transform");
    /// ```
    fn try_transform<F, U, E>(self, f: F) -> crate::combinators::ZTryTransform<Self, F, U>
    where
        F: Fn(Self::Output) -> Result<U, E>,
        E: std::fmt::Display,
    {
        crate::combinators::ZTryTransform::new(self, f)
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
