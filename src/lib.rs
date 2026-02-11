//! # vld — Type-safe runtime validation for Rust
//!
//! `vld` is a validation library inspired by [Zod](https://zod.dev/) that combines
//! schema definition with type-safe parsing.
//!
//! ## Quick Start
//!
//! ```rust
//! use vld::prelude::*;
//!
//! // Define a validated struct
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct User {
//!         pub name: String => vld::string().min(2).max(50),
//!         pub email: String => vld::string().email(),
//!         pub age: Option<i64> => vld::number().int().gte(18).optional(),
//!     }
//! }
//!
//! // Parse from JSON string
//! let user = User::parse(r#"{"name": "Alex", "email": "alex@example.com"}"#).unwrap();
//! assert_eq!(user.name, "Alex");
//! assert_eq!(user.age, None);
//! ```

pub mod collections;
pub mod combinators;
#[cfg(feature = "diff")]
pub mod diff;
pub mod error;
pub mod format;
pub mod i18n;
pub mod input;
#[cfg(feature = "openapi")]
pub mod json_schema;
mod macros;
pub mod modifiers;
pub mod object;
pub mod primitives;
pub mod schema;

// Re-export serde_json for use in macros
#[doc(hidden)]
pub use serde_json;

// Re-export serde for DynSchema users
#[doc(hidden)]
pub use serde;

// ---------------------------------------------------------------------------
// Feature-gate helper macros.
//
// These are conditionally compiled based on **vld's** features, so when a
// foreign crate invokes `vld::schema!`, the check uses vld's features
// instead of the consumer's — eliminating `unexpected_cfgs` warnings.
// ---------------------------------------------------------------------------

/// Emit the given tokens only when the `serialize` feature is enabled on `vld`.
#[cfg(feature = "serialize")]
#[doc(hidden)]
#[macro_export]
macro_rules! __vld_if_serialize {
    ($($tt:tt)*) => { $($tt)* };
}

/// No-op: `serialize` feature is disabled.
#[cfg(not(feature = "serialize"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __vld_if_serialize {
    ($($tt:tt)*) => {};
}

/// Emit the given tokens only when the `openapi` feature is enabled on `vld`.
#[cfg(feature = "openapi")]
#[doc(hidden)]
#[macro_export]
macro_rules! __vld_if_openapi {
    ($($tt:tt)*) => { $($tt)* };
}

/// No-op: `openapi` feature is disabled.
#[cfg(not(feature = "openapi"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __vld_if_openapi {
    ($($tt:tt)*) => {};
}

// Re-export regex_lite when the `regex` feature is enabled
#[cfg(feature = "regex")]
pub use regex_lite;

// Re-export the derive macro when the `derive` feature is enabled
#[cfg(feature = "derive")]
pub use vld_derive::Validate;

// ---------------------------------------------------------------------------
// Convenience constructors
// ---------------------------------------------------------------------------

/// Create a string validation schema.
pub fn string() -> primitives::ZString {
    primitives::ZString::new()
}

/// Create a number validation schema (`f64`).
pub fn number() -> primitives::ZNumber {
    primitives::ZNumber::new()
}

/// Create a boolean validation schema.
pub fn boolean() -> primitives::ZBoolean {
    primitives::ZBoolean::new()
}

/// Create an array validation schema.
pub fn array<T: schema::VldSchema>(element: T) -> collections::ZArray<T> {
    collections::ZArray::new(element)
}

/// Create a dynamic object validation schema.
///
/// For type-safe objects, prefer the [`schema!`] macro.
pub fn object() -> object::ZObject {
    object::ZObject::new()
}

/// Create a schema for nested/composed structs.
pub fn nested<T, F>(f: F) -> schema::NestedSchema<T, F>
where
    F: Fn(&serde_json::Value) -> Result<T, error::VldError>,
{
    schema::NestedSchema::new(f)
}

/// Create a literal value schema. Validates exact match.
///
/// ```
/// use vld::prelude::*;
/// assert_eq!(vld::literal("admin").parse(r#""admin""#).unwrap(), "admin");
/// assert!(vld::literal(42i64).parse("42").is_ok());
/// assert!(vld::literal(true).parse("true").is_ok());
/// ```
pub fn literal<T: primitives::IntoLiteral>(val: T) -> primitives::ZLiteral<T> {
    primitives::ZLiteral::new(val)
}

/// Create a string enum schema. Validates against a fixed set of values.
///
/// ```
/// use vld::prelude::*;
/// let role = vld::enumeration(&["admin", "user", "mod"]);
/// assert!(role.parse(r#""admin""#).is_ok());
/// assert!(role.parse(r#""hacker""#).is_err());
/// ```
pub fn enumeration(variants: &[&str]) -> primitives::ZEnum {
    primitives::ZEnum::new(variants)
}

/// Create a schema that accepts any JSON value.
pub fn any() -> primitives::ZAny {
    primitives::ZAny::new()
}

/// Create a date validation schema. Parses ISO 8601 date strings (`YYYY-MM-DD`)
/// into `chrono::NaiveDate`.
///
/// Requires the `chrono` feature.
#[cfg(feature = "chrono")]
pub fn date() -> primitives::ZDate {
    primitives::ZDate::new()
}

/// Create a datetime validation schema. Parses ISO 8601 datetime strings
/// into `chrono::DateTime<chrono::Utc>`.
///
/// Requires the `chrono` feature.
#[cfg(feature = "chrono")]
pub fn datetime() -> primitives::ZDateTime {
    primitives::ZDateTime::new()
}

/// Create a record (dictionary) schema. All values validated by the given schema.
///
/// ```
/// use vld::prelude::*;
/// let schema = vld::record(vld::number().positive());
/// let map = schema.parse(r#"{"a": 1, "b": 2}"#).unwrap();
/// assert_eq!(map.len(), 2);
/// ```
pub fn record<V: schema::VldSchema>(value_schema: V) -> collections::ZRecord<V> {
    collections::ZRecord::new(value_schema)
}

/// Create a Map schema. Validates `[[key, value], ...]` arrays into `HashMap`.
pub fn map<K: schema::VldSchema, V: schema::VldSchema>(
    key_schema: K,
    value_schema: V,
) -> collections::ZMap<K, V> {
    collections::ZMap::new(key_schema, value_schema)
}

/// Create a Set schema. Validates arrays into `HashSet` (unique elements).
pub fn set<T: schema::VldSchema>(element: T) -> collections::ZSet<T> {
    collections::ZSet::new(element)
}

/// Create a union of two schemas. Returns `Either<A, B>`.
pub fn union<A: schema::VldSchema, B: schema::VldSchema>(a: A, b: B) -> combinators::ZUnion2<A, B> {
    combinators::ZUnion2::new(a, b)
}

/// Create a union of three schemas. Returns `Either3<A, B, C>`.
pub fn union3<A: schema::VldSchema, B: schema::VldSchema, C: schema::VldSchema>(
    a: A,
    b: B,
    c: C,
) -> combinators::ZUnion3<A, B, C> {
    combinators::ZUnion3::new(a, b, c)
}

/// Create an intersection of two schemas (input must satisfy both).
///
/// Both schemas run on the same input. The output of the first schema is returned.
pub fn intersection<A: schema::VldSchema, B: schema::VldSchema>(
    a: A,
    b: B,
) -> combinators::ZIntersection<A, B> {
    combinators::ZIntersection::new(a, b)
}

/// Create a discriminated union schema.
///
/// Routes to the correct variant schema based on the value of a discriminator field.
pub fn discriminated_union(discriminator: impl Into<String>) -> combinators::ZDiscriminatedUnion {
    combinators::ZDiscriminatedUnion::new(discriminator)
}

/// Create a lazy schema for recursive data structures.
///
/// The factory function is called on each parse, enabling self-referencing schemas.
pub fn lazy<T: schema::VldSchema, F: Fn() -> T>(factory: F) -> combinators::ZLazy<T, F> {
    combinators::ZLazy::new(factory)
}

/// Create a schema from a custom validation function.
///
/// The function receives a `&serde_json::Value` and returns `Result<T, String>`.
pub fn custom<F, T>(check: F) -> combinators::ZCustom<F, T>
where
    F: Fn(&serde_json::Value) -> Result<T, String>,
{
    combinators::ZCustom::new(check)
}

/// Preprocess the JSON value before passing it to a schema.
pub fn preprocess<F, S>(preprocessor: F, schema: S) -> combinators::ZPreprocess<F, S>
where
    F: Fn(&serde_json::Value) -> serde_json::Value,
    S: schema::VldSchema,
{
    combinators::ZPreprocess::new(preprocessor, schema)
}

// ---------------------------------------------------------------------------
// Prelude
// ---------------------------------------------------------------------------

/// Common imports for working with `vld`.
pub mod prelude {
    pub use crate::collections::{ZArray, ZMap, ZRecord, ZSet};
    pub use crate::combinators::{
        Either, Either3, ZCatch, ZCustom, ZDescribe, ZDiscriminatedUnion, ZIntersection, ZLazy,
        ZMessage, ZPipe, ZPreprocess, ZRefine, ZSuperRefine, ZTransform, ZUnion2, ZUnion3,
    };
    pub use crate::error::{
        FieldResult, IssueBuilder, IssueCode, ParseResult, PathSegment, ValidationIssue, VldError,
    };
    pub use crate::format::{flatten_error, prettify_error, treeify_error};
    pub use crate::input::VldInput;
    #[cfg(feature = "openapi")]
    pub use crate::json_schema::JsonSchema;
    pub use crate::modifiers::{ZDefault, ZNullable, ZNullish, ZOptional};
    pub use crate::object::ZObject;
    pub use crate::primitives::{
        IntoLiteral, ZAny, ZBoolean, ZEnum, ZInt, ZLiteral, ZNumber, ZString,
    };
    #[cfg(feature = "chrono")]
    pub use crate::primitives::{ZDate, ZDateTime};
    pub use crate::schema::{VldParse, VldSchema};
}
