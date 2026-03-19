//! # vld-surrealdb — SurrealDB integration for the `vld` validation library
//!
//! Validate JSON documents **before** sending to SurrealDB and **after** receiving.
//!
//! Zero dependency on `surrealdb` crate — works purely through `serde`, so it's
//! compatible with any SurrealDB SDK version (2.x, 3.x, etc.).
//!
//! ## Quick Start
//!
//! ```rust
//! use vld_surrealdb::prelude::*;
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct PersonSchema {
//!         pub name: String  => vld::string().min(1).max(100),
//!         pub email: String => vld::string().email(),
//!         pub age: i64      => vld::number().int().min(0).max(150),
//!     }
//! }
//!
//! #[derive(serde::Serialize)]
//! struct Person { name: String, email: String, age: i64 }
//!
//! let person = Person {
//!     name: "Alice".into(),
//!     email: "alice@example.com".into(),
//!     age: 30,
//! };
//!
//! // Validate before db.create("person").content(person)
//! validate_content::<PersonSchema, _>(&person).unwrap();
//! ```

use std::fmt;
use std::ops::Deref;

pub use vld;

// ========================= Error type ========================================

/// Error returned by `vld-surrealdb` operations.
#[derive(Debug, Clone)]
pub enum VldSurrealError {
    /// Schema validation failed.
    Validation(vld::error::VldError),
    /// Failed to serialize the value to JSON for validation.
    Serialization(String),
}

impl fmt::Display for VldSurrealError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VldSurrealError::Validation(e) => write!(f, "Validation error: {}", e),
            VldSurrealError::Serialization(e) => write!(f, "Serialization error: {}", e),
        }
    }
}

impl std::error::Error for VldSurrealError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VldSurrealError::Validation(e) => Some(e),
            VldSurrealError::Serialization(_) => None,
        }
    }
}

impl From<vld::error::VldError> for VldSurrealError {
    fn from(e: vld::error::VldError) -> Self {
        VldSurrealError::Validation(e)
    }
}

/// Structured error with field-level details, serializable as JSON.
///
/// Useful for returning validation errors from SurrealDB server functions
/// or custom endpoints.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

/// Serializable validation error for API responses.
///
/// Converts from [`VldSurrealError`] and can be sent over the wire.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VldSurrealResponse {
    pub error: String,
    pub fields: Vec<FieldError>,
}

impl VldSurrealResponse {
    /// Create from a validation error.
    pub fn from_vld_error(e: &vld::error::VldError) -> Self {
        let fields = e
            .issues
            .iter()
            .map(|issue: &vld::error::ValidationIssue| FieldError {
                field: issue
                    .path
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join("."),
                message: issue.message.clone(),
            })
            .collect();

        Self {
            error: "Validation failed".to_string(),
            fields,
        }
    }

    /// Create from a [`VldSurrealError`].
    pub fn from_error(e: &VldSurrealError) -> Self {
        match e {
            VldSurrealError::Validation(ve) => Self::from_vld_error(ve),
            VldSurrealError::Serialization(msg) => Self {
                error: "Serialization error".to_string(),
                fields: vec![FieldError {
                    field: String::new(),
                    message: msg.clone(),
                }],
            },
        }
    }

    /// Convert to a JSON value.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

// ========================= Validated<S, T> ===================================

/// A wrapper that proves its inner value has been validated against schema `S`.
///
/// Implements `Serialize` so it can be passed directly to SurrealDB operations.
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
/// let v = vld_surrealdb::Validated::<NameSchema, _>::new(row).unwrap();
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
    pub fn new(value: T) -> Result<Self, VldSurrealError> {
        let json = serde_json::to_value(&value)
            .map_err(|e| VldSurrealError::Serialization(e.to_string()))?;
        S::vld_parse_value(&json).map_err(VldSurrealError::Validation)?;
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

impl<S, T: serde::Serialize> serde::Serialize for Validated<S, T> {
    fn serialize<Ser: serde::Serializer>(&self, serializer: Ser) -> Result<Ser::Ok, Ser::Error> {
        self.inner.serialize(serializer)
    }
}

// ========================= Standalone helpers ================================

/// Validate a value against schema `S` before create/insert/update.
///
/// Use before `db.create("table").content(value)` or `db.insert("table").content(value)`.
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
/// struct Item { name: String, qty: i64 }
///
/// let item = Item { name: "Widget".into(), qty: 5 };
/// vld_surrealdb::validate_content::<ItemSchema, _>(&item).unwrap();
/// ```
pub fn validate_content<S, T>(value: &T) -> Result<(), VldSurrealError>
where
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    let json =
        serde_json::to_value(value).map_err(|e| VldSurrealError::Serialization(e.to_string()))?;
    S::vld_parse_value(&json).map_err(VldSurrealError::Validation)?;
    Ok(())
}

/// Validate raw JSON value against schema `S`.
///
/// Useful for validating SurrealQL query results or raw JSON payloads.
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
/// let json = serde_json::json!({"name": "Alice"});
/// vld_surrealdb::validate_json::<NameSchema>(&json).unwrap();
/// ```
pub fn validate_json<S>(value: &serde_json::Value) -> Result<(), VldSurrealError>
where
    S: vld::schema::VldParse,
{
    S::vld_parse_value(value).map_err(VldSurrealError::Validation)?;
    Ok(())
}

/// Validate a record loaded from SurrealDB against schema `S`.
///
/// Use after `db.select("table")` to enforce invariants on stored data.
pub fn validate_record<S, T>(value: &T) -> Result<(), VldSurrealError>
where
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    validate_content::<S, T>(value)
}

/// Validate a batch of records against schema `S`.
///
/// Returns the index and error of the first invalid record.
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
/// assert!(vld_surrealdb::validate_records::<NameSchema, _>(&rows).is_ok());
/// ```
pub fn validate_records<S, T>(rows: &[T]) -> Result<(), (usize, VldSurrealError)>
where
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    for (i, row) in rows.iter().enumerate() {
        validate_record::<S, T>(row).map_err(|e| (i, e))?;
    }
    Ok(())
}

/// Validate a single field value against a vld schema instance.
///
/// Useful for validating individual fields in SurrealDB `MERGE` operations.
///
/// ```
/// use vld::prelude::*;
///
/// let schema = vld::string().min(1).max(100);
/// let val = serde_json::json!("Bob");
/// assert!(vld_surrealdb::validate_value(&schema, &val).is_ok());
///
/// let bad = serde_json::json!("");
/// assert!(vld_surrealdb::validate_value(&schema, &bad).is_err());
/// ```
pub fn validate_value<S: vld::schema::VldSchema>(
    schema: &S,
    value: &serde_json::Value,
) -> Result<(), VldSurrealError> {
    schema
        .parse_value(value)
        .map(|_| ())
        .map_err(VldSurrealError::Validation)
}

// ========================= VldText<S> ========================================

/// A validated text field for SurrealDB documents.
///
/// Wraps a `String` and validates on construction and deserialization.
/// The schema must have a field named `value`.
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
/// let email = vld_surrealdb::VldText::<EmailField>::new("user@example.com").unwrap();
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

impl<S: vld::schema::VldParse> VldText<S> {
    pub fn new(input: impl Into<String>) -> Result<Self, VldSurrealError> {
        let s = input.into();
        let json = serde_json::json!({ "value": s });
        S::vld_parse_value(&json).map_err(VldSurrealError::Validation)?;
        Ok(Self {
            value: s,
            _schema: std::marker::PhantomData,
        })
    }

    pub fn new_unchecked(input: impl Into<String>) -> Self {
        Self {
            value: input.into(),
            _schema: std::marker::PhantomData,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

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

// ========================= VldInt<S> =========================================

/// A validated integer field for SurrealDB documents.
///
/// Wraps an `i64` and validates on construction and deserialization.
/// The schema must have a field named `value`.
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
/// let age = vld_surrealdb::VldInt::<AgeField>::new(25).unwrap();
/// assert_eq!(*age, 25);
/// assert!(vld_surrealdb::VldInt::<AgeField>::new(-1).is_err());
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

impl<S: vld::schema::VldParse> VldInt<S> {
    pub fn new(input: i64) -> Result<Self, VldSurrealError> {
        let json = serde_json::json!({ "value": input });
        S::vld_parse_value(&json).map_err(VldSurrealError::Validation)?;
        Ok(Self {
            value: input,
            _schema: std::marker::PhantomData,
        })
    }

    pub fn new_unchecked(input: i64) -> Self {
        Self {
            value: input,
            _schema: std::marker::PhantomData,
        }
    }

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

// ========================= VldFloat<S> =======================================

/// A validated float field for SurrealDB documents.
///
/// Wraps an `f64` and validates on construction and deserialization.
/// The schema must have a field named `value`.
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
/// let price = vld_surrealdb::VldFloat::<PriceField>::new(9.99).unwrap();
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
    pub fn new(input: f64) -> Result<Self, VldSurrealError> {
        let json = serde_json::json!({ "value": input });
        S::vld_parse_value(&json).map_err(VldSurrealError::Validation)?;
        Ok(Self {
            value: input,
            _schema: std::marker::PhantomData,
        })
    }

    pub fn new_unchecked(input: f64) -> Self {
        Self {
            value: input,
            _schema: std::marker::PhantomData,
        }
    }

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

// ========================= VldBool<S> ========================================

/// A validated boolean field for SurrealDB documents.
///
/// The schema must have a field named `value`.
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
/// let active = vld_surrealdb::VldBool::<ActiveField>::new(true).unwrap();
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
    pub fn new(input: bool) -> Result<Self, VldSurrealError> {
        let json = serde_json::json!({ "value": input });
        S::vld_parse_value(&json).map_err(VldSurrealError::Validation)?;
        Ok(Self {
            value: input,
            _schema: std::marker::PhantomData,
        })
    }

    pub fn new_unchecked(input: bool) -> Self {
        Self {
            value: input,
            _schema: std::marker::PhantomData,
        }
    }

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

// ========================= Macro =============================================

/// Validate multiple fields inline before a SurrealDB operation.
///
/// ```
/// use vld::prelude::*;
/// use vld_surrealdb::validate_fields;
///
/// let name = "Alice";
/// let email = "alice@example.com";
///
/// let result = validate_fields! {
///     name => vld::string().min(1).max(100),
///     email => vld::string().email(),
/// };
/// assert!(result.is_ok());
/// ```
#[macro_export]
macro_rules! validate_fields {
    ($($field:ident => $schema:expr),* $(,)?) => {{
        let mut __vld_errors: Vec<$crate::FieldError> = Vec::new();
        $(
            {
                let __vld_val = ::serde_json::to_value(&$field).ok();
                let __vld_schema = $schema;
                if let Some(ref val) = __vld_val {
                    use $crate::vld::schema::VldSchema;
                    if let Err(_) = __vld_schema.validate(val) {
                        let msg = format!("field '{}' failed validation", stringify!($field));
                        __vld_errors.push($crate::FieldError {
                            field: stringify!($field).to_string(),
                            message: msg,
                        });
                    }
                }
            }
        )*
        if __vld_errors.is_empty() {
            ::std::result::Result::Ok::<(), $crate::VldSurrealError>(())
        } else {
            let mut __vld_err = $crate::vld::error::VldError::new();
            for fe in &__vld_errors {
                __vld_err.issues.push($crate::vld::error::ValidationIssue {
                    code: $crate::vld::error::IssueCode::Custom {
                        code: fe.field.clone(),
                    },
                    message: fe.message.clone(),
                    path: vec![
                        $crate::vld::error::PathSegment::Field(fe.field.clone()),
                    ],
                    received: None,
                });
            }
            ::std::result::Result::Err($crate::VldSurrealError::Validation(__vld_err))
        }
    }};
}

// ========================= Prelude ===========================================

pub mod prelude {
    pub use crate::{
        validate_content, validate_fields, validate_json, validate_record, validate_records,
        validate_value, FieldError, Validated, VldBool, VldFloat, VldInt, VldSurrealError,
        VldSurrealResponse, VldText,
    };
    pub use vld::prelude::*;
}
