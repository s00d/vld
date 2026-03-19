//! # vld-dioxus — Dioxus integration for the `vld` validation library
//!
//! Shared validation for Dioxus server functions and WASM clients.
//! Define validation rules once, use them on both the server and the browser.
//!
//! **Zero dependency on `dioxus`** — works with any Dioxus version (0.5, 0.6, 0.7+).
//! Compatible with WASM and native targets.
//!
//! ## Key Features
//!
//! | Feature | Description |
//! |---|---|
//! | [`validate_args!`] | Inline validation of server function arguments |
//! | [`validate`] / [`validate_value`] | Validate a serializable value against a schema |
//! | [`check_field`] | Single-field validation for reactive UI |
//! | [`check_all_fields`] | Multi-field validation returning per-field errors |
//! | [`VldServerError`] | Serializable error type for server→client error transport |
//!
//! ## Quick Example
//!
//! ```ignore
//! // Shared validation rules (used on server AND client)
//! fn name_schema() -> vld::primitives::ZString { vld::string().min(2).max(50) }
//! fn email_schema() -> vld::primitives::ZString { vld::string().email() }
//!
//! // Server function
//! #[server]
//! async fn create_user(name: String, email: String) -> Result<(), ServerFnError> {
//!     vld_dioxus::validate_args! {
//!         name  => name_schema(),
//!         email => email_schema(),
//!     }.map_err(|e| ServerFnError::new(e.to_string()))?;
//!     Ok(())
//! }
//!
//! // Client component (reactive validation)
//! #[component]
//! fn CreateUserForm() -> Element {
//!     let mut name = use_signal(String::new);
//!     let name_err = use_memo(move || {
//!         vld_dioxus::check_field(&name(), &name_schema())
//!     });
//!     // ... render form with error display
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;

// ========================= Error types =======================================

/// A single field validation error.
///
/// Designed to be serialized across the server→client boundary
/// and displayed in form UIs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FieldError {
    /// Field name (matches the server function argument name or struct field).
    pub field: String,
    /// Human-readable error message.
    pub message: String,
}

/// Structured validation error for Dioxus server functions.
///
/// Serializable and deserializable — can be transmitted from server to client
/// as part of a `ServerFnError` message or custom error type.
///
/// # With `ServerFnError`
///
/// ```ignore
/// #[server]
/// async fn my_fn(name: String) -> Result<(), ServerFnError> {
///     vld_dioxus::validate_args! {
///         name => vld::string().min(2),
///     }.map_err(|e| ServerFnError::new(e.to_string()))?;
///     Ok(())
/// }
/// ```
///
/// # With custom error type
///
/// ```ignore
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// enum AppError {
///     Validation(VldServerError),
///     Server(String),
/// }
///
/// impl FromServerFnError for AppError { /* ... */ }
///
/// #[server]
/// async fn my_fn(name: String) -> Result<(), AppError> {
///     vld_dioxus::validate_args! {
///         name => vld::string().min(2),
///     }.map_err(AppError::Validation)?;
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VldServerError {
    /// Summary message.
    pub message: String,
    /// Per-field validation errors.
    pub fields: Vec<FieldError>,
}

impl VldServerError {
    /// Create a validation error from a list of field errors.
    pub fn validation(fields: Vec<FieldError>) -> Self {
        let count = fields.len();
        Self {
            message: format!(
                "Validation failed: {} field{} invalid",
                count,
                if count == 1 { "" } else { "s" }
            ),
            fields,
        }
    }

    /// Create an internal/serialization error.
    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            fields: Vec::new(),
        }
    }

    /// Get the error message for a specific field.
    pub fn field_error(&self, field: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|f| f.field == field)
            .map(|f| f.message.as_str())
    }

    /// Get all error messages for a specific field.
    pub fn field_errors(&self, field: &str) -> Vec<&str> {
        self.fields
            .iter()
            .filter(|f| f.field == field)
            .map(|f| f.message.as_str())
            .collect()
    }

    /// Check if a specific field has errors.
    pub fn has_field_error(&self, field: &str) -> bool {
        self.fields.iter().any(|f| f.field == field)
    }

    /// List all field names that have errors.
    pub fn error_fields(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.fields.iter().map(|f| f.field.as_str()).collect();
        names.dedup();
        names
    }

    /// Parse a `VldServerError` from a JSON string (e.g. from `ServerFnError` message).
    ///
    /// Returns `None` if the string is not a valid `VldServerError` JSON.
    pub fn from_json(s: &str) -> Option<Self> {
        serde_json::from_str(s).ok()
    }
}

impl fmt::Display for VldServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match serde_json::to_string(self) {
            Ok(json) => write!(f, "{}", json),
            Err(_) => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for VldServerError {}

impl From<vld::error::VldError> for VldServerError {
    fn from(error: vld::error::VldError) -> Self {
        let fields: Vec<FieldError> = error
            .issues
            .iter()
            .map(|issue| {
                let field = issue
                    .path
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(".");
                FieldError {
                    field,
                    message: issue.message.clone(),
                }
            })
            .collect();
        Self::validation(fields)
    }
}

// ========================= Schema-based validation ===========================

/// Validate a serializable value against a vld schema type.
///
/// Serializes `data` to JSON, then validates using `S::vld_parse_value()`.
///
/// # Example
///
/// ```ignore
/// vld::schema! {
///     struct UserInput {
///         name: String => vld::string().min(2),
///         email: String => vld::string().email(),
///     }
/// }
///
/// #[derive(Serialize)]
/// struct Args { name: String, email: String }
///
/// vld_dioxus::validate::<UserInput, _>(&Args { name, email })?;
/// ```
pub fn validate<S, T>(data: &T) -> Result<(), VldServerError>
where
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    let json = serde_json::to_value(data)
        .map_err(|e| VldServerError::internal(format!("Serialization error: {}", e)))?;
    S::vld_parse_value(&json).map_err(VldServerError::from)?;
    Ok(())
}

/// Validate a `serde_json::Value` against a vld schema type.
pub fn validate_value<S>(json: &serde_json::Value) -> Result<(), VldServerError>
where
    S: vld::schema::VldParse,
{
    S::vld_parse_value(json).map_err(VldServerError::from)?;
    Ok(())
}

// ========================= Field-level validation ============================

/// Check a single value against a schema, returning an error message if invalid.
///
/// Designed for reactive client-side validation in Dioxus components.
/// Returns `None` if valid, `Some(message)` if invalid.
///
/// # Example
///
/// ```
/// let error = vld_dioxus::check_field(&"A".to_string(), &vld::string().min(2));
/// assert!(error.is_some());
///
/// let error = vld_dioxus::check_field(&"Alice".to_string(), &vld::string().min(2));
/// assert!(error.is_none());
/// ```
pub fn check_field<V, S>(value: &V, schema: &S) -> Option<String>
where
    V: serde::Serialize,
    S: vld::schema::VldSchema,
{
    let json = serde_json::to_value(value).ok()?;
    match schema.parse_value(&json) {
        Ok(_) => None,
        Err(e) => e.issues.first().map(|i| i.message.clone()),
    }
}

/// Check a single value, returning all error messages (not just the first).
///
/// # Example
///
/// ```
/// let errors = vld_dioxus::check_field_all(&"".to_string(), &vld::string().min(2).email());
/// assert!(errors.len() >= 1);
/// ```
pub fn check_field_all<V, S>(value: &V, schema: &S) -> Vec<String>
where
    V: serde::Serialize,
    S: vld::schema::VldSchema,
{
    let json = match serde_json::to_value(value) {
        Ok(j) => j,
        Err(_) => return vec![],
    };
    match schema.parse_value(&json) {
        Ok(_) => vec![],
        Err(e) => e.issues.iter().map(|i| i.message.clone()).collect(),
    }
}

/// Validate all fields of a serializable struct against a vld schema type.
///
/// Returns a list of [`FieldError`]s (empty if all fields are valid).
/// Designed for validating entire form state at once.
///
/// # Example
///
/// ```
/// use vld_dioxus::FieldError;
///
/// vld::schema! {
///     struct UserSchema {
///         name: String => vld::string().min(2),
///         email: String => vld::string().email(),
///     }
/// }
///
/// #[derive(serde::Serialize)]
/// struct FormData { name: String, email: String }
///
/// let data = FormData { name: "A".into(), email: "bad".into() };
/// let errors = vld_dioxus::check_all_fields::<UserSchema, _>(&data);
/// assert!(!errors.is_empty());
/// assert!(errors.iter().any(|e| e.field == ".name"));
/// ```
pub fn check_all_fields<S, T>(data: &T) -> Vec<FieldError>
where
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    let json = match serde_json::to_value(data) {
        Ok(j) => j,
        Err(_) => return vec![],
    };
    match S::vld_parse_value(&json) {
        Ok(_) => vec![],
        Err(e) => e
            .issues
            .iter()
            .map(|i| FieldError {
                field: i
                    .path
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join("."),
                message: i.message.clone(),
            })
            .collect(),
    }
}

// ========================= Macro =============================================

/// Validate server function arguments inline.
///
/// Each argument is validated against its schema. All errors are accumulated
/// and returned as a [`VldServerError`].
///
/// # Example
///
/// ```ignore
/// #[server]
/// async fn create_user(name: String, email: String, age: i64) -> Result<(), ServerFnError> {
///     vld_dioxus::validate_args! {
///         name  => vld::string().min(2).max(50),
///         email => vld::string().email(),
///         age   => vld::number().int().min(0).max(150),
///     }.map_err(|e| ServerFnError::new(e.to_string()))?;
///
///     // ... all arguments are valid
///     Ok(())
/// }
/// ```
///
/// # Shared Schemas
///
/// Define schema factories to share between server and client:
///
/// ```ignore
/// // shared.rs — compiles for both server and WASM
/// pub fn name_schema() -> impl vld::schema::VldSchema<Output = String> {
///     vld::string().min(2).max(50)
/// }
///
/// // server function
/// vld_dioxus::validate_args! {
///     name => shared::name_schema(),
/// }.map_err(|e| ServerFnError::new(e.to_string()))?;
///
/// // client component (use_memo)
/// let err = use_memo(move || vld_dioxus::check_field(&name(), &shared::name_schema()));
/// ```
#[macro_export]
macro_rules! validate_args {
    ($($field:ident => $schema:expr),* $(,)?) => {{
        use ::vld::schema::VldSchema as _;
        let mut __vld_field_errors: ::std::vec::Vec<$crate::FieldError> = ::std::vec::Vec::new();

        $(
            {
                let __vld_json = ::vld::serde_json::to_value(&$field)
                    .unwrap_or(::vld::serde_json::Value::Null);
                if let ::std::result::Result::Err(e) = ($schema).parse_value(&__vld_json) {
                    for issue in &e.issues {
                        __vld_field_errors.push($crate::FieldError {
                            field: ::std::string::String::from(stringify!($field)),
                            message: issue.message.clone(),
                        });
                    }
                }
            }
        )*

        if __vld_field_errors.is_empty() {
            ::std::result::Result::Ok::<(), $crate::VldServerError>(())
        } else {
            ::std::result::Result::Err($crate::VldServerError::validation(__vld_field_errors))
        }
    }};
}

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{
        check_all_fields, check_field, check_field_all, validate, validate_args, validate_value,
        FieldError, VldServerError,
    };
    pub use vld::prelude::*;
}
