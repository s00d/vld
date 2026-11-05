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

pub use vld;

// ========================= Error type ========================================

vld_integration_common::__vld_db_error!(
    /// Error returned by `vld-surrealdb` operations.
    VldSurrealError
);

/// A single field error (`field` is a clean path such as `user.email` or `items[0]`).
pub use vld_integration_common::form::FieldError;

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
        let fields = vld_integration_common::flatten_issues(e)
            .into_iter()
            .map(|(field, message)| FieldError { field, message })
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

vld_integration_common::__vld_db_validated!(VldSurrealError);

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
    __vld_validate::<S, T>(value).map(|_| ())
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

// ========================= Column wrappers ===================================
//
// `VldText`, `VldInt`, `VldFloat`, `VldBool`: validated on construction and on
// deserialization; the schema must have a field named `value`.
//
// ```
// vld::schema! {
//     #[derive(Debug)]
//     pub struct EmailField { pub value: String => vld::string().email() }
// }
// let email = vld_surrealdb::VldText::<EmailField>::new("user@example.com").unwrap();
// ```

vld_integration_common::__vld_db_wrappers! {
    VldSurrealError;
    text: []
    int: []
    float: []
    bool: []
}

#[doc(hidden)]
pub fn __validate_field<V, S>(
    name: &str,
    value: &V,
    schema: &S,
    out: &mut vld::error::VldError,
) -> Result<(), VldSurrealError>
where
    V: serde::Serialize + ?Sized,
    S: vld::schema::VldSchema,
{
    let json = serde_json::to_value(value)
        .map_err(|e| VldSurrealError::Serialization(format!("field '{}': {}", name, e)))?;
    if let Err(e) = schema.parse_value(&json) {
        for mut issue in e.issues {
            issue
                .path
                .insert(0, vld::error::PathSegment::Field(name.to_string()));
            out.issues.push(issue);
        }
    }
    Ok(())
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
///
/// Each field's real validation issues are returned, with the field name
/// prepended to their path. A field that cannot be serialized yields
/// [`VldSurrealError::Serialization`].
#[macro_export]
macro_rules! validate_fields {
    ($($field:ident => $schema:expr),* $(,)?) => {{
        let mut __vld_err = $crate::vld::error::VldError::new();
        let mut __vld_ser: ::std::option::Option<$crate::VldSurrealError> = None;
        $(
            if __vld_ser.is_none() {
                if let ::std::result::Result::Err(e) = $crate::__validate_field(
                    stringify!($field), &$field, &($schema), &mut __vld_err,
                ) {
                    __vld_ser = Some(e);
                }
            }
        )*
        match __vld_ser {
            Some(e) => ::std::result::Result::Err(e),
            None if __vld_err.issues.is_empty() => {
                ::std::result::Result::Ok::<(), $crate::VldSurrealError>(())
            }
            None => ::std::result::Result::Err($crate::VldSurrealError::Validation(__vld_err)),
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
