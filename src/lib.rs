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
//!
//! ## Module map
//!
//! - [`mod@schema`] — the [`VldSchema`](schema::VldSchema) trait, [`VldParse`](schema::VldParse),
//!   [`DynSchema`](schema::DynSchema), input sources and nested schemas
//! - [`error`] — [`VldError`](error::VldError), [`IssueCode`](error::IssueCode),
//!   formatting ([`error::format`]) and translations ([`error::i18n`])
//! - [`primitives`], [`collections`], [`combinators`], [`modifiers`], [`mod@object`] — schema types
//! - `openapi` (feature `openapi`) — JSON Schema / OpenAPI generation and schema diffs
//! - [`prelude`] — everything needed for day-to-day use

pub mod collections;
pub mod combinators;
pub mod error;
mod macros;
pub mod modifiers;
pub mod object;
#[cfg(feature = "openapi")]
pub mod openapi;
pub mod prelude;
pub mod primitives;
pub mod schema;

#[doc(hidden)]
#[path = "private.rs"]
pub mod __private;

// Old module paths (pre-0.5), kept as aliases.
pub use error::{format, i18n};
#[cfg(feature = "openapi")]
pub use openapi as json_schema;
#[cfg(feature = "diff")]
pub use openapi::diff;
pub use schema::input;

// Date backends: additive, each in its own module.
#[cfg(feature = "chrono")]
pub use primitives::date::chrono;
#[cfg(feature = "jiff")]
pub use primitives::date::jiff;
#[cfg(feature = "time")]
pub use primitives::date::time;

// Re-export serde_json for use in macros
#[doc(hidden)]
pub use serde_json;

// Re-export serde for DynSchema users
#[doc(hidden)]
pub use serde;

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

macro_rules! int_constructors {
    ($($name:ident: $t:ty),+ $(,)?) => {$(
        #[doc = concat!("Create a `", stringify!($t), "` integer schema (exact, range-checked; see [`primitives::ZInteger`]).")]
        pub fn $name() -> primitives::ZInteger<$t> {
            primitives::ZInteger::new()
        }
    )+};
}

int_constructors! {
    i8: i8, i16: i16, i32: i32, i64: i64, isize: isize,
    u8: u8, u16: u16, u32: u32, u64: u64, usize: usize,
}

/// Create an `f32` number schema (rejects values outside the `f32` range).
pub fn f32() -> primitives::ZF32 {
    primitives::ZF32::new()
}

/// Create a boolean validation schema.
pub fn boolean() -> primitives::ZBoolean {
    primitives::ZBoolean::new()
}

/// Create a bytes validation schema (`Vec<u8>`).
///
/// By default expects a JSON array of byte integers `0..=255`.
/// Use `.base64()` to additionally accept Base64 string input.
pub fn bytes() -> primitives::ZBytes {
    primitives::ZBytes::new()
}

/// Create a decimal validation schema (`rust_decimal::Decimal`).
#[cfg(feature = "decimal")]
pub fn decimal() -> primitives::ZDecimal {
    primitives::ZDecimal::new()
}

/// Create an IP network/CIDR schema (`ipnet::IpNet`).
#[cfg(feature = "net")]
pub fn ip_network() -> primitives::ZIpNetwork {
    primitives::ZIpNetwork::new()
}

/// Create a socket address schema (`std::net::SocketAddr`).
pub fn socket_addr() -> primitives::ZSocketAddr {
    primitives::ZSocketAddr::new()
}

/// Create a raw JSON value schema with shape constraints.
pub fn json_value() -> primitives::ZJsonValue {
    primitives::ZJsonValue::new()
}

/// Create a file validation schema.
///
/// Requires the `file` feature.
#[cfg(feature = "file")]
pub fn file() -> primitives::ZFile {
    primitives::ZFile::new()
}

/// Create a duration validation schema (`std::time::Duration`).
pub fn duration() -> primitives::ZDuration {
    primitives::ZDuration::new()
}

/// Create a path validation schema (`std::path::PathBuf`).
pub fn path() -> primitives::ZPath {
    primitives::ZPath::new()
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

/// Create a **named** nested schema. Usually called through [`nested!`].
///
/// - `name` — OpenAPI component name; the field's JSON Schema becomes
///   `{"$ref": "#/components/schemas/{name}"}`. Must match `[A-Za-z0-9._-]+`.
/// - `f` — parser for the nested value (e.g. `Address::parse_value`).
/// - `json_schema_fn` — full JSON Schema of the nested type, registered as the
///   component (`None` disables registration).
/// - `collect_nested_fn` — collects the nested type's own nested schemas so
///   grandchildren are registered too.
///
/// With `json_schema_fn` set, `to_openapi_document()` of the parent and the
/// `vld-utoipa` / `vld-aide` / `vld-schemars` integrations register the
/// component automatically.
pub fn nested_named<T, F>(
    name: &'static str,
    f: F,
    json_schema_fn: Option<fn() -> serde_json::Value>,
    collect_nested_fn: Option<schema::NestedSchemaCollectFn>,
) -> schema::NestedSchema<T, F>
where
    F: Fn(&serde_json::Value) -> Result<T, error::VldError>,
{
    schema::NestedSchema::new_named(f, name, json_schema_fn, collect_nested_fn)
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

/// Deserialize the JSON value into `T` with serde (Rust enums, `uuid::Uuid`,
/// `url::Url`, `semver::Version`, …).
///
/// Serde errors become a `parse_error` issue carrying the serde message.
/// The JSON Schema is `{}` unless set with [`ZFromSerde::schema`](primitives::ZFromSerde::schema).
///
/// ```
/// use vld::prelude::*;
///
/// #[derive(Debug, PartialEq, serde::Deserialize)]
/// #[serde(rename_all = "snake_case")]
/// enum Status { Active, OnHold }
///
/// let schema = vld::from_serde::<Status>()
///     .schema(serde_json::json!({"type": "string", "enum": ["active", "on_hold"]}));
/// assert_eq!(schema.parse(r#""on_hold""#).unwrap(), Status::OnHold);
/// assert!(schema.parse(r#""gone""#).is_err());
/// ```
pub fn from_serde<T: serde::de::DeserializeOwned>() -> primitives::ZFromSerde<T> {
    primitives::ZFromSerde::new()
}

/// Parse a JSON string with `T: FromStr` (`IpAddr`, `Uuid`, `Url`, numbers
/// as strings, …).
///
/// Parse errors become an `invalid_format` (`from_str`) issue whose message is
/// the error's `Display`. The JSON Schema is `{"type": "string"}` unless set
/// with [`ZFromStr::schema`](primitives::ZFromStr::schema).
///
/// ```
/// use vld::prelude::*;
/// use std::net::IpAddr;
///
/// let schema = vld::from_str::<IpAddr>()
///     .schema(serde_json::json!({"type": "string", "format": "ip"}));
/// let ip = schema.parse(r#""::1""#).unwrap();
/// assert!(ip.is_loopback());
/// assert!(schema.parse(r#""not-an-ip""#).is_err());
/// ```
pub fn from_str<T>() -> primitives::ZFromStr<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    primitives::ZFromStr::new()
}

/// Map fixed strings to values of `T` — a light alternative to
/// [`from_serde()`] for enums without `Deserialize`.
///
/// ```
/// use vld::prelude::*;
///
/// #[derive(Debug, Clone, PartialEq)]
/// enum Level { Low, High }
///
/// let level = vld::enum_of(&[("low", Level::Low), ("high", Level::High)]);
/// assert_eq!(level.parse(r#""high""#).unwrap(), Level::High);
/// let err = level.parse(r#""mid""#).unwrap_err();
/// assert_eq!(err.issues[0].code.key(), "invalid_enum_value");
/// ```
pub fn enum_of<T: Clone>(variants: &[(&str, T)]) -> primitives::ZEnumOf<T> {
    primitives::ZEnumOf::new(variants)
}

/// Create a schema that accepts any JSON value.
pub fn any() -> primitives::ZAny {
    primitives::ZAny::new()
}

/// Create a date schema with the only enabled date backend.
///
/// Available only when exactly one of `chrono`, `jiff`, `time` is enabled;
/// otherwise use `vld::chrono::date()`, `vld::jiff::date()` or
/// `vld::time::date()` (see [`primitives::date`]).
#[cfg(any(
    all(feature = "chrono", not(feature = "jiff"), not(feature = "time")),
    all(feature = "jiff", not(feature = "chrono"), not(feature = "time")),
    all(feature = "time", not(feature = "chrono"), not(feature = "jiff"))
))]
pub fn date() -> primitives::ZDate {
    primitives::ZDate::new()
}

/// Create a datetime schema with the only enabled date backend.
///
/// Available only when exactly one of `chrono`, `jiff`, `time` is enabled;
/// otherwise use `vld::chrono::datetime()` etc. (see [`primitives::date`]).
#[cfg(any(
    all(feature = "chrono", not(feature = "jiff"), not(feature = "time")),
    all(feature = "jiff", not(feature = "chrono"), not(feature = "time")),
    all(feature = "time", not(feature = "chrono"), not(feature = "jiff"))
))]
pub fn datetime() -> primitives::ZDateTime {
    primitives::ZDateTime::new()
}

/// Create a time-of-day schema (`HH:MM:SS[.frac]`) with the only enabled
/// date backend.
///
/// Available only when exactly one of `chrono`, `jiff`, `time` is enabled;
/// otherwise use `vld::chrono::time()`, `vld::jiff::time()` or
/// `vld::time::time()` (see [`primitives::date`]).
#[cfg(any(
    all(feature = "chrono", not(feature = "jiff"), not(feature = "time")),
    all(feature = "jiff", not(feature = "chrono"), not(feature = "time")),
    all(feature = "time", not(feature = "chrono"), not(feature = "jiff"))
))]
pub fn time_of_day() -> primitives::ZTime {
    primitives::ZTime::new()
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
