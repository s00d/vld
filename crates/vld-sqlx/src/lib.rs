//! # vld-sqlx — SQLx integration for the `vld` validation library
//!
//! Validate data **before** inserting into the database, and use strongly-typed
//! validated column types with SQLx.
//!
//! ## Quick Start
//!
//! ```ignore
//! use vld_sqlx::prelude::*;
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct NewUserSchema {
//!         pub name: String  => vld::string().min(1).max(100),
//!         pub email: String => vld::string().email(),
//!     }
//! }
//!
//! #[derive(serde::Serialize)]
//! struct NewUser { name: String, email: String }
//!
//! let user = NewUser { name: "Alice".into(), email: "alice@example.com".into() };
//! validate_insert::<NewUserSchema, _>(&user)?;
//! // now safe to insert via sqlx::query!(...)
//! ```

use std::fmt;
use std::ops::Deref;

pub use vld;

// ========================= Error type ========================================

/// Error returned by `vld-sqlx` operations.
#[derive(Debug, Clone)]
pub enum VldSqlxError {
    /// Schema validation failed.
    Validation(vld::error::VldError),
    /// Failed to serialize the value to JSON for validation.
    Serialization(String),
}

impl fmt::Display for VldSqlxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VldSqlxError::Validation(e) => write!(f, "Validation error: {}", e),
            VldSqlxError::Serialization(e) => write!(f, "Serialization error: {}", e),
        }
    }
}

impl std::error::Error for VldSqlxError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VldSqlxError::Validation(e) => Some(e),
            VldSqlxError::Serialization(_) => None,
        }
    }
}

impl From<vld::error::VldError> for VldSqlxError {
    fn from(e: vld::error::VldError) -> Self {
        VldSqlxError::Validation(e)
    }
}

impl From<VldSqlxError> for sqlx::Error {
    fn from(e: VldSqlxError) -> Self {
        sqlx::Error::Protocol(e.to_string())
    }
}

// ========================= Validated<S, T> ===================================

/// A wrapper that proves its inner value has been validated against schema `S`.
///
/// `S` must implement [`vld::schema::VldParse`] and `T` must be
/// [`serde::Serialize`] so the value can be converted to JSON for validation.
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
/// let v = vld_sqlx::Validated::<NameSchema, _>::new(row).unwrap();
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
    pub fn new(value: T) -> Result<Self, VldSqlxError> {
        let json =
            serde_json::to_value(&value).map_err(|e| VldSqlxError::Serialization(e.to_string()))?;
        S::vld_parse_value(&json).map_err(VldSqlxError::Validation)?;
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

// ========================= Standalone helpers ================================

/// Validate a value against schema `S` before inserting.
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
/// vld_sqlx::validate_insert::<ItemSchema, _>(&item).unwrap();
/// ```
pub fn validate_insert<S, T>(value: &T) -> Result<(), VldSqlxError>
where
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    let json =
        serde_json::to_value(value).map_err(|e| VldSqlxError::Serialization(e.to_string()))?;
    S::vld_parse_value(&json).map_err(VldSqlxError::Validation)?;
    Ok(())
}

/// Alias for [`validate_insert`] — same logic applies to updates.
pub fn validate_update<S, T>(value: &T) -> Result<(), VldSqlxError>
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
pub fn validate_row<S, T>(value: &T) -> Result<(), VldSqlxError>
where
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    validate_insert::<S, T>(value)
}

/// Validate a batch of rows against schema `S`.
///
/// Returns the index and error of the first invalid row.
///
/// ```
/// use vld::prelude::*;
///
/// vld::schema! {
///     #[derive(Debug)]
///     pub struct NameSchema {
///         pub name: String => vld::string().min(1),
///     }
/// }
///
/// #[derive(serde::Serialize)]
/// struct Row { name: String }
///
/// let rows = vec![
///     Row { name: "Alice".into() },
///     Row { name: "Bob".into() },
/// ];
/// assert!(vld_sqlx::validate_rows::<NameSchema, _>(&rows).is_ok());
/// ```
pub fn validate_rows<S, T>(rows: &[T]) -> Result<(), (usize, VldSqlxError)>
where
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    for (i, row) in rows.iter().enumerate() {
        validate_row::<S, T>(row).map_err(|e| (i, e))?;
    }
    Ok(())
}

// ========================= VldText<S> ========================================

/// A validated text column type.
///
/// Wraps a `String` and ensures it passes the vld schema `S` on construction.
/// Implements SQLx `Type`, `Encode`, `Decode` for any database where `String` does,
/// so it works seamlessly with `sqlx::query!`, `FromRow`, etc.
///
/// The schema must have a field named `value`.
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
/// let email = vld_sqlx::VldText::<EmailField>::new("user@example.com").unwrap();
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
    pub fn new(input: impl Into<String>) -> Result<Self, VldSqlxError> {
        let s = input.into();
        let json = serde_json::json!({ "value": s });
        S::vld_parse_value(&json).map_err(VldSqlxError::Validation)?;
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

// SQLx Type — delegates to String
impl<S, DB: sqlx::Database> sqlx::Type<DB> for VldText<S>
where
    String: sqlx::Type<DB>,
{
    fn type_info() -> DB::TypeInfo {
        <String as sqlx::Type<DB>>::type_info()
    }

    fn compatible(ty: &DB::TypeInfo) -> bool {
        <String as sqlx::Type<DB>>::compatible(ty)
    }
}

// SQLx Encode — delegates to String
impl<'q, S, DB: sqlx::Database> sqlx::Encode<'q, DB> for VldText<S>
where
    String: sqlx::Encode<'q, DB>,
{
    fn encode_by_ref(
        &self,
        buf: &mut <DB as sqlx::Database>::ArgumentBuffer<'q>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        self.value.encode_by_ref(buf)
    }
}

// SQLx Decode — decodes as String without re-validation (trusted DB data)
impl<'r, S: vld::schema::VldParse, DB: sqlx::Database> sqlx::Decode<'r, DB> for VldText<S>
where
    String: sqlx::Decode<'r, DB>,
{
    fn decode(
        value: <DB as sqlx::Database>::ValueRef<'r>,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <String as sqlx::Decode<'r, DB>>::decode(value)?;
        Ok(Self::new_unchecked(s))
    }
}

// ========================= VldInt<S> =========================================

/// A validated integer column type.
///
/// Wraps an `i64` and ensures it passes the vld schema `S` on construction.
/// Implements SQLx `Type`, `Encode`, `Decode` for any database where `i64` does.
///
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
/// let age = vld_sqlx::VldInt::<AgeField>::new(25).unwrap();
/// assert_eq!(*age, 25);
/// assert!(vld_sqlx::VldInt::<AgeField>::new(-1).is_err());
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
    pub fn new(input: i64) -> Result<Self, VldSqlxError> {
        let json = serde_json::json!({ "value": input });
        S::vld_parse_value(&json).map_err(VldSqlxError::Validation)?;
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

// SQLx Type — delegates to i64
impl<S, DB: sqlx::Database> sqlx::Type<DB> for VldInt<S>
where
    i64: sqlx::Type<DB>,
{
    fn type_info() -> DB::TypeInfo {
        <i64 as sqlx::Type<DB>>::type_info()
    }

    fn compatible(ty: &DB::TypeInfo) -> bool {
        <i64 as sqlx::Type<DB>>::compatible(ty)
    }
}

// SQLx Encode — delegates to i64
impl<'q, S, DB: sqlx::Database> sqlx::Encode<'q, DB> for VldInt<S>
where
    i64: sqlx::Encode<'q, DB>,
{
    fn encode_by_ref(
        &self,
        buf: &mut <DB as sqlx::Database>::ArgumentBuffer<'q>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        self.value.encode_by_ref(buf)
    }
}

// SQLx Decode — decodes as i64 without re-validation (trusted DB data)
impl<'r, S: vld::schema::VldParse, DB: sqlx::Database> sqlx::Decode<'r, DB> for VldInt<S>
where
    i64: sqlx::Decode<'r, DB>,
{
    fn decode(
        value: <DB as sqlx::Database>::ValueRef<'r>,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        let v = <i64 as sqlx::Decode<'r, DB>>::decode(value)?;
        Ok(Self::new_unchecked(v))
    }
}

// ========================= VldBool<S> ========================================

/// A validated boolean column type.
///
/// Wraps a `bool` and ensures it passes the vld schema `S` on construction.
/// The schema must have a field named `value`.
///
/// # Example
///
/// ```
/// use vld::prelude::*;
///
/// vld::schema! {
///     #[derive(Debug)]
///     pub struct ActiveField {
///         pub value: bool => vld::boolean(),
///     }
/// }
///
/// let active = vld_sqlx::VldBool::<ActiveField>::new(true).unwrap();
/// assert_eq!(*active, true);
/// ```
pub struct VldBool<S> {
    value: bool,
    _schema: std::marker::PhantomData<S>,
}

impl<S> Clone for VldBool<S> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<S> Copy for VldBool<S> {}

impl<S> PartialEq for VldBool<S> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl<S> Eq for VldBool<S> {}

impl<S: vld::schema::VldParse> VldBool<S> {
    /// Create a validated boolean.
    pub fn new(input: bool) -> Result<Self, VldSqlxError> {
        let json = serde_json::json!({ "value": input });
        S::vld_parse_value(&json).map_err(VldSqlxError::Validation)?;
        Ok(Self {
            value: input,
            _schema: std::marker::PhantomData,
        })
    }

    /// Create without validation.
    pub fn new_unchecked(input: bool) -> Self {
        Self {
            value: input,
            _schema: std::marker::PhantomData,
        }
    }

    /// Get the inner value.
    pub fn get(&self) -> bool {
        self.value
    }
}

impl<S> Deref for VldBool<S> {
    type Target = bool;
    fn deref(&self) -> &bool {
        &self.value
    }
}

impl<S> fmt::Debug for VldBool<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VldBool({})", self.value)
    }
}

impl<S> fmt::Display for VldBool<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl<S> serde::Serialize for VldBool<S> {
    fn serialize<Ser: serde::Serializer>(&self, serializer: Ser) -> Result<Ser::Ok, Ser::Error> {
        self.value.serialize(serializer)
    }
}

impl<'de, S: vld::schema::VldParse> serde::Deserialize<'de> for VldBool<S> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = bool::deserialize(deserializer)?;
        VldBool::<S>::new(v).map_err(serde::de::Error::custom)
    }
}

// SQLx Type — delegates to bool
impl<S, DB: sqlx::Database> sqlx::Type<DB> for VldBool<S>
where
    bool: sqlx::Type<DB>,
{
    fn type_info() -> DB::TypeInfo {
        <bool as sqlx::Type<DB>>::type_info()
    }

    fn compatible(ty: &DB::TypeInfo) -> bool {
        <bool as sqlx::Type<DB>>::compatible(ty)
    }
}

impl<'q, S, DB: sqlx::Database> sqlx::Encode<'q, DB> for VldBool<S>
where
    bool: sqlx::Encode<'q, DB>,
{
    fn encode_by_ref(
        &self,
        buf: &mut <DB as sqlx::Database>::ArgumentBuffer<'q>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        self.value.encode_by_ref(buf)
    }
}

impl<'r, S: vld::schema::VldParse, DB: sqlx::Database> sqlx::Decode<'r, DB> for VldBool<S>
where
    bool: sqlx::Decode<'r, DB>,
{
    fn decode(
        value: <DB as sqlx::Database>::ValueRef<'r>,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        let v = <bool as sqlx::Decode<'r, DB>>::decode(value)?;
        Ok(Self::new_unchecked(v))
    }
}

// ========================= VldFloat<S> =======================================

/// A validated float column type.
///
/// Wraps an `f64` and ensures it passes the vld schema `S` on construction.
/// The schema must have a field named `value`.
///
/// # Example
///
/// ```
/// use vld::prelude::*;
///
/// vld::schema! {
///     #[derive(Debug)]
///     pub struct PriceField {
///         pub value: f64 => vld::number().min(0.0),
///     }
/// }
///
/// let price = vld_sqlx::VldFloat::<PriceField>::new(9.99).unwrap();
/// assert!((*price - 9.99).abs() < f64::EPSILON);
/// ```
pub struct VldFloat<S> {
    value: f64,
    _schema: std::marker::PhantomData<S>,
}

impl<S> Clone for VldFloat<S> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<S> Copy for VldFloat<S> {}

impl<S> PartialEq for VldFloat<S> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<S: vld::schema::VldParse> VldFloat<S> {
    /// Create a validated float.
    pub fn new(input: f64) -> Result<Self, VldSqlxError> {
        let json = serde_json::json!({ "value": input });
        S::vld_parse_value(&json).map_err(VldSqlxError::Validation)?;
        Ok(Self {
            value: input,
            _schema: std::marker::PhantomData,
        })
    }

    /// Create without validation.
    pub fn new_unchecked(input: f64) -> Self {
        Self {
            value: input,
            _schema: std::marker::PhantomData,
        }
    }

    /// Get the inner value.
    pub fn get(&self) -> f64 {
        self.value
    }
}

impl<S> Deref for VldFloat<S> {
    type Target = f64;
    fn deref(&self) -> &f64 {
        &self.value
    }
}

impl<S> fmt::Debug for VldFloat<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VldFloat({})", self.value)
    }
}

impl<S> fmt::Display for VldFloat<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl<S> serde::Serialize for VldFloat<S> {
    fn serialize<Ser: serde::Serializer>(&self, serializer: Ser) -> Result<Ser::Ok, Ser::Error> {
        self.value.serialize(serializer)
    }
}

impl<'de, S: vld::schema::VldParse> serde::Deserialize<'de> for VldFloat<S> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = f64::deserialize(deserializer)?;
        VldFloat::<S>::new(v).map_err(serde::de::Error::custom)
    }
}

// SQLx Type — delegates to f64
impl<S, DB: sqlx::Database> sqlx::Type<DB> for VldFloat<S>
where
    f64: sqlx::Type<DB>,
{
    fn type_info() -> DB::TypeInfo {
        <f64 as sqlx::Type<DB>>::type_info()
    }

    fn compatible(ty: &DB::TypeInfo) -> bool {
        <f64 as sqlx::Type<DB>>::compatible(ty)
    }
}

impl<'q, S, DB: sqlx::Database> sqlx::Encode<'q, DB> for VldFloat<S>
where
    f64: sqlx::Encode<'q, DB>,
{
    fn encode_by_ref(
        &self,
        buf: &mut <DB as sqlx::Database>::ArgumentBuffer<'q>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        self.value.encode_by_ref(buf)
    }
}

impl<'r, S: vld::schema::VldParse, DB: sqlx::Database> sqlx::Decode<'r, DB> for VldFloat<S>
where
    f64: sqlx::Decode<'r, DB>,
{
    fn decode(
        value: <DB as sqlx::Database>::ValueRef<'r>,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        let v = <f64 as sqlx::Decode<'r, DB>>::decode(value)?;
        Ok(Self::new_unchecked(v))
    }
}

// ========================= Prelude ===========================================

pub mod prelude {
    pub use crate::{
        validate_insert, validate_row, validate_rows, validate_update, Validated, VldBool,
        VldFloat, VldInt, VldSqlxError, VldText,
    };
    pub use vld::prelude::*;
}
