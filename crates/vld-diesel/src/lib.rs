//! # vld-diesel — Diesel integration for the `vld` validation library
//!
//! Validate data **before** inserting into the database, and use strongly-typed
//! validated column types.
//!
//! ## Quick Start
//!
//! ```ignore
//! use vld_diesel::prelude::*;
//!
//! // 1. Define a vld schema for your insertable struct
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct NewUserSchema {
//!         pub name: String  => vld::string().min(1).max(100),
//!         pub email: String => vld::string().email(),
//!     }
//! }
//!
//! // 2. Wrap your Diesel Insertable with Validated
//! let new_user = NewUser { name: "Alice".into(), email: "alice@example.com".into() };
//! let validated = Validated::<NewUserSchema, _>::new(new_user)?;
//!
//! // 3. Insert — the inner value is guaranteed to be valid
//! diesel::insert_into(users::table)
//!     .values(validated.inner())
//!     .execute(&mut conn)?;
//! ```
//!
//! ## Validated column types
//!
//! Use [`VldText`] for columns that must always satisfy a validation schema:
//!
//! ```ignore
//! use vld_diesel::VldText;
//!
//! let email = VldText::<EmailSchema>::new("user@example.com")?;
//! ```

use std::fmt;
use std::ops::Deref;

// ---------------------------------------------------------------------------
// Re-exports
// ---------------------------------------------------------------------------

pub use vld;

// ---------------------------------------------------------------------------
// Validated<S, T> — wrapper that ensures T passes schema S
// ---------------------------------------------------------------------------

/// A wrapper that proves its inner value has been validated against schema `S`.
///
/// `S` must implement [`vld::schema::VldParse`] and `T` must be
/// [`serde::Serialize`] so the value can be converted to JSON for validation.
///
/// Once constructed (via [`new`](Self::new)), the inner `T` is guaranteed valid.
///
/// # Example
///
/// ```
/// use vld::prelude::*;
///
/// vld::schema! {
///     #[derive(Debug)]
///     pub struct NameSchema {
///         pub name: String => vld::string().min(1).max(50),
///     }
/// }
///
/// #[derive(serde::Serialize)]
/// struct Row { name: String }
///
/// let row = Row { name: "Alice".into() };
/// let v = vld_diesel::Validated::<NameSchema, _>::new(row).unwrap();
/// assert_eq!(v.inner().name, "Alice");
/// ```
pub struct Validated<S, T> {
    inner: T,
    _schema: std::marker::PhantomData<S>,
}

impl<S, T> Validated<S, T>
where
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    /// Validate `value` against schema `S` and wrap it on success.
    pub fn new(value: T) -> Result<Self, VldDieselError> {
        let json = serde_json::to_value(&value)
            .map_err(|e| VldDieselError::Serialization(e.to_string()))?;
        S::vld_parse_value(&json).map_err(VldDieselError::Validation)?;
        Ok(Self {
            inner: value,
            _schema: std::marker::PhantomData,
        })
    }

    /// Get a reference to the validated inner value.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Consume and return the inner value.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<S, T> Deref for Validated<S, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<S, T: fmt::Debug> fmt::Debug for Validated<S, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Validated")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<S, T: Clone> Clone for Validated<S, T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _schema: std::marker::PhantomData,
        }
    }
}

// ---------------------------------------------------------------------------
// validate_insert / validate_update — standalone helpers
// ---------------------------------------------------------------------------

/// Validate a value against schema `S` before inserting.
///
/// This is a standalone function — use it when you don't need the
/// [`Validated`] wrapper.
///
/// ```
/// use vld::prelude::*;
///
/// vld::schema! {
///     #[derive(Debug)]
///     pub struct ItemSchema {
///         pub name: String => vld::string().min(1),
///         pub qty: i64 => vld::number().int().min(0),
///     }
/// }
///
/// #[derive(serde::Serialize)]
/// struct NewItem { name: String, qty: i64 }
///
/// let item = NewItem { name: "Widget".into(), qty: 5 };
/// vld_diesel::validate_insert::<ItemSchema, _>(&item).unwrap();
/// ```
pub fn validate_insert<S, T>(value: &T) -> Result<(), VldDieselError>
where
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    let json =
        serde_json::to_value(value).map_err(|e| VldDieselError::Serialization(e.to_string()))?;
    S::vld_parse_value(&json).map_err(VldDieselError::Validation)?;
    Ok(())
}

/// Alias for [`validate_insert`] — same logic applies to updates.
pub fn validate_update<S, T>(value: &T) -> Result<(), VldDieselError>
where
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    validate_insert::<S, T>(value)
}

/// Validate a row loaded from the database against schema `S`.
///
/// Useful to enforce invariants on data that may have been inserted
/// by other systems or before validation was in place.
pub fn validate_row<S, T>(value: &T) -> Result<(), VldDieselError>
where
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    validate_insert::<S, T>(value)
}

// ---------------------------------------------------------------------------
// VldText<S> — a validated String column type
// ---------------------------------------------------------------------------

/// A validated text column type.
///
/// `VldText<S>` wraps a `String` and ensures it passes the `vld` schema `S`
/// on construction. It implements Diesel's `ToSql`/`FromSql` for the `Text`
/// SQL type, so you can use it directly in Diesel models.
///
/// `S` must implement [`vld::schema::VldParse`]. The schema must have a
/// field named `value`.
///
/// # Example
///
/// ```
/// use vld::prelude::*;
///
/// vld::schema! {
///     #[derive(Debug)]
///     pub struct EmailField {
///         pub value: String => vld::string().email(),
///     }
/// }
///
/// let email = vld_diesel::VldText::<EmailField>::new("user@example.com").unwrap();
/// assert_eq!(email.as_str(), "user@example.com");
/// ```
pub struct VldText<S> {
    value: String,
    _schema: std::marker::PhantomData<S>,
}

impl<S> Clone for VldText<S> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            _schema: std::marker::PhantomData,
        }
    }
}

impl<S> PartialEq for VldText<S> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl<S> Eq for VldText<S> {}

impl<S> PartialOrd for VldText<S> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<S> Ord for VldText<S> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value.cmp(&other.value)
    }
}

impl<S> std::hash::Hash for VldText<S> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl<S: vld::schema::VldParse> VldText<S> {
    /// Create a validated text value.
    ///
    /// The `input` is wrapped in `{"value": "..."}` and validated against `S`.
    pub fn new(input: impl Into<String>) -> Result<Self, VldDieselError> {
        let s = input.into();
        let json = serde_json::json!({ "value": s });
        S::vld_parse_value(&json).map_err(VldDieselError::Validation)?;
        Ok(Self {
            value: s,
            _schema: std::marker::PhantomData,
        })
    }

    /// Create without validation (e.g. for data loaded from a trusted DB).
    pub fn new_unchecked(input: impl Into<String>) -> Self {
        Self {
            value: input.into(),
            _schema: std::marker::PhantomData,
        }
    }

    /// Get the inner string.
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// Consume and return the inner `String`.
    pub fn into_inner(self) -> String {
        self.value
    }
}

impl<S> Deref for VldText<S> {
    type Target = str;
    fn deref(&self) -> &str {
        &self.value
    }
}

impl<S> AsRef<str> for VldText<S> {
    fn as_ref(&self) -> &str {
        &self.value
    }
}

impl<S> fmt::Debug for VldText<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VldText({:?})", self.value)
    }
}

impl<S> fmt::Display for VldText<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.value)
    }
}

impl<S> serde::Serialize for VldText<S> {
    fn serialize<Ser: serde::Serializer>(&self, serializer: Ser) -> Result<Ser::Ok, Ser::Error> {
        self.value.serialize(serializer)
    }
}

impl<'de, S: vld::schema::VldParse> serde::Deserialize<'de> for VldText<S> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        VldText::<S>::new(&s).map_err(serde::de::Error::custom)
    }
}

// Diesel ToSql / FromSql for VldText — maps to diesel::sql_types::Text.
// We delegate to the underlying String / &str.

impl<S, DB> diesel::serialize::ToSql<diesel::sql_types::Text, DB> for VldText<S>
where
    DB: diesel::backend::Backend,
    str: diesel::serialize::ToSql<diesel::sql_types::Text, DB>,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, DB>,
    ) -> diesel::serialize::Result {
        <str as diesel::serialize::ToSql<diesel::sql_types::Text, DB>>::to_sql(&self.value, out)
    }
}

impl<S: vld::schema::VldParse, DB> diesel::deserialize::FromSql<diesel::sql_types::Text, DB>
    for VldText<S>
where
    DB: diesel::backend::Backend,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, DB>,
{
    fn from_sql(
        bytes: <DB as diesel::backend::Backend>::RawValue<'_>,
    ) -> diesel::deserialize::Result<Self> {
        let s =
            <String as diesel::deserialize::FromSql<diesel::sql_types::Text, DB>>::from_sql(bytes)?;
        Ok(Self::new_unchecked(s))
    }
}

// ---------------------------------------------------------------------------
// VldInt<S> — a validated integer column type
// ---------------------------------------------------------------------------

/// A validated integer column type.
///
/// Wraps an `i64` and ensures it passes the `vld` schema `S` on construction.
/// The schema must have a field named `value`.
///
/// # Example
///
/// ```
/// use vld::prelude::*;
///
/// vld::schema! {
///     #[derive(Debug)]
///     pub struct AgeField {
///         pub value: i64 => vld::number().int().min(0).max(150),
///     }
/// }
///
/// let age = vld_diesel::VldInt::<AgeField>::new(25).unwrap();
/// assert_eq!(*age, 25);
/// assert!(vld_diesel::VldInt::<AgeField>::new(-1).is_err());
/// ```
pub struct VldInt<S> {
    value: i64,
    _schema: std::marker::PhantomData<S>,
}

impl<S> Clone for VldInt<S> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<S> Copy for VldInt<S> {}

impl<S> PartialEq for VldInt<S> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl<S> Eq for VldInt<S> {}

impl<S> PartialOrd for VldInt<S> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<S> Ord for VldInt<S> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value.cmp(&other.value)
    }
}

impl<S> std::hash::Hash for VldInt<S> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl<S: vld::schema::VldParse> VldInt<S> {
    /// Create a validated integer.
    pub fn new(input: i64) -> Result<Self, VldDieselError> {
        let json = serde_json::json!({ "value": input });
        S::vld_parse_value(&json).map_err(VldDieselError::Validation)?;
        Ok(Self {
            value: input,
            _schema: std::marker::PhantomData,
        })
    }

    /// Create without validation.
    pub fn new_unchecked(input: i64) -> Self {
        Self {
            value: input,
            _schema: std::marker::PhantomData,
        }
    }

    /// Get the inner value.
    pub fn get(&self) -> i64 {
        self.value
    }
}

impl<S> Deref for VldInt<S> {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.value
    }
}

impl<S> fmt::Debug for VldInt<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VldInt({})", self.value)
    }
}

impl<S> fmt::Display for VldInt<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl<S> serde::Serialize for VldInt<S> {
    fn serialize<Ser: serde::Serializer>(&self, serializer: Ser) -> Result<Ser::Ok, Ser::Error> {
        self.value.serialize(serializer)
    }
}

impl<'de, S: vld::schema::VldParse> serde::Deserialize<'de> for VldInt<S> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = i64::deserialize(deserializer)?;
        VldInt::<S>::new(v).map_err(serde::de::Error::custom)
    }
}

// Diesel ToSql / FromSql for VldInt — maps to diesel::sql_types::BigInt (i64).

impl<S, DB> diesel::serialize::ToSql<diesel::sql_types::BigInt, DB> for VldInt<S>
where
    DB: diesel::backend::Backend,
    i64: diesel::serialize::ToSql<diesel::sql_types::BigInt, DB>,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, DB>,
    ) -> diesel::serialize::Result {
        <i64 as diesel::serialize::ToSql<diesel::sql_types::BigInt, DB>>::to_sql(&self.value, out)
    }
}

impl<S: vld::schema::VldParse, DB> diesel::deserialize::FromSql<diesel::sql_types::BigInt, DB>
    for VldInt<S>
where
    DB: diesel::backend::Backend,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, DB>,
{
    fn from_sql(
        bytes: <DB as diesel::backend::Backend>::RawValue<'_>,
    ) -> diesel::deserialize::Result<Self> {
        let v =
            <i64 as diesel::deserialize::FromSql<diesel::sql_types::BigInt, DB>>::from_sql(bytes)?;
        Ok(Self::new_unchecked(v))
    }
}

// FromSql for Integer (i32) — useful when the DB column is INTEGER.

impl<S: vld::schema::VldParse, DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>
    for VldInt<S>
where
    DB: diesel::backend::Backend,
    i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
{
    fn from_sql(
        bytes: <DB as diesel::backend::Backend>::RawValue<'_>,
    ) -> diesel::deserialize::Result<Self> {
        let v =
            <i32 as diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>>::from_sql(bytes)?;
        Ok(Self::new_unchecked(v as i64))
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error returned by `vld-diesel` operations.
#[derive(Debug, Clone)]
pub enum VldDieselError {
    /// Schema validation failed.
    Validation(vld::error::VldError),
    /// Failed to serialize the value to JSON for validation.
    Serialization(String),
}

impl fmt::Display for VldDieselError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VldDieselError::Validation(e) => write!(f, "Validation error: {}", e),
            VldDieselError::Serialization(e) => write!(f, "Serialization error: {}", e),
        }
    }
}

impl std::error::Error for VldDieselError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VldDieselError::Validation(e) => Some(e),
            VldDieselError::Serialization(_) => None,
        }
    }
}

impl From<vld::error::VldError> for VldDieselError {
    fn from(e: vld::error::VldError) -> Self {
        VldDieselError::Validation(e)
    }
}

// ---------------------------------------------------------------------------
// Prelude
// ---------------------------------------------------------------------------

pub mod prelude {
    pub use crate::{
        validate_insert, validate_row, validate_update, Validated, VldDieselError, VldInt, VldText,
    };
    pub use vld::prelude::*;
}
