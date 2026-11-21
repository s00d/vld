//! Form and server-function validation: field-level errors for UIs and a
//! serializable error for server → client transport.
//!
//! Framework-agnostic (no Leptos / Dioxus dependency, so any version works);
//! builds for native and WASM targets. Define the rules once, check them on
//! the server and in the browser.
//!
//! | Item | Use |
//! |---|---|
//! | [`validate_args!`] | Validate server-function arguments inline |
//! | [`validate`] / [`validate_value`] | Validate a value against a schema type |
//! | [`check_field`] / [`check_field_all`] | Single-field validation for reactive UI |
//! | [`check_all_fields`] | Whole-form validation returning per-field errors |
//! | [`VldServerError`] | Serializable error for server → client transport |
//!
//! Field paths have no leading dot (`name`, `user.email`, `items[0]`).
//!
//! ```
//! vld::schema! {
//!     struct CreateUserSchema {
//!         name: String => vld::string().min(2).max(50),
//!         email: String => vld::string().email(),
//!     }
//! }
//!
//! #[derive(serde::Serialize)]
//! struct CreateUserArgs { name: String, email: String }
//!
//! let args = CreateUserArgs { name: "X".into(), email: "bad".into() };
//! let errors = vld::form::check_all_fields::<CreateUserSchema, _>(&args);
//! assert_eq!(errors.len(), 2);
//! assert_eq!(errors[0].field, "name");
//! ```
//!
//! Server-function arguments, and the error read back on the client:
//!
//! ```
//! fn name_schema() -> vld::primitives::ZString { vld::string().min(2).max(50) }
//! fn email_schema() -> vld::primitives::ZString { vld::string().email() }
//!
//! let name = "A".to_string();
//! let email = "bad".to_string();
//! let err = vld::form::validate_args! {
//!     name  => name_schema(),
//!     email => email_schema(),
//! }
//! .unwrap_err();
//! assert_eq!(err.error_fields(), ["name", "email"]);
//!
//! // The client side gets it back from `ServerFnError`'s message:
//! let shown = format!("error running server function: {err}");
//! let back = vld::form::VldServerError::from_server_message(&shown).unwrap();
//! assert!(back.has_field_error("email"));
//!
//! assert!(vld::form::check_field(&"Alice".to_string(), &name_schema()).is_none());
//! ```
//!
//! In a Leptos app (Dioxus is the same with `use_signal` / `use_memo`):
//!
//! ```ignore
//! // Shared validation rules (used on server AND client)
//! fn name_schema() -> vld::primitives::ZString { vld::string().min(2).max(50) }
//! fn email_schema() -> vld::primitives::ZString { vld::string().email() }
//!
//! // Server function
//! #[server]
//! async fn create_user(name: String, email: String) -> Result<(), ServerFnError> {
//!     vld::form::validate_args! {
//!         name  => name_schema(),
//!         email => email_schema(),
//!     }.map_err(|e| ServerFnError::new(e.to_string()))?;
//!     // ... create user in database
//!     Ok(())
//! }
//!
//! // Client component (reactive validation)
//! #[component]
//! fn CreateUserForm() -> impl IntoView {
//!     let (name, set_name) = signal(String::new());
//!     let name_err = Memo::new(move |_| {
//!         vld::form::check_field(&name.get(), &name_schema())
//!     });
//!     // ... render form with error display
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;

/// A single field validation error.
///
/// `field` is a clean dotted path (`name`, `user.email`, `items[0]`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FieldError {
    /// Field path (matches the server function argument name or struct field).
    pub field: String,
    /// Human-readable error message.
    pub message: String,
}

/// Structured, serializable validation error for server functions.
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

    /// Get the first error message for a specific field.
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

    /// List all field names that have errors (unique, in first-seen order).
    pub fn error_fields(&self) -> Vec<&str> {
        let mut seen = std::collections::HashSet::new();
        self.fields
            .iter()
            .map(|f| f.field.as_str())
            .filter(|f| seen.insert(*f))
            .collect()
    }

    /// Parse a `VldServerError` from a JSON string (e.g. from a `ServerFnError` message).
    pub fn from_json(s: &str) -> Option<Self> {
        serde_json::from_str(s).ok()
    }

    /// Parse a `VldServerError` out of a server-function error message.
    ///
    /// Leptos / Dioxus `ServerFnError`'s `Display` prefixes the message
    /// (`"error running server function: {...}"`), so [`from_json`](Self::from_json)
    /// on `e.to_string()` never matches; this reads the JSON object from the
    /// first `{` to the last `}`.
    ///
    /// ```
    /// use vld::form::{FieldError, VldServerError};
    /// let e = VldServerError::validation(vec![FieldError { field: "name".into(), message: "too short".into() }]);
    /// let shown = format!("error running server function: {e}");
    /// let back = VldServerError::from_server_message(&shown).unwrap();
    /// assert_eq!(back.field_error("name"), Some("too short"));
    /// ```
    pub fn from_server_message(s: &str) -> Option<Self> {
        let (start, end) = (s.find('{')?, s.rfind('}')?);
        Self::from_json(s.get(start..=end)?)
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

impl From<crate::error::VldError> for VldServerError {
    fn from(error: crate::error::VldError) -> Self {
        Self::validation(field_errors(&error))
    }
}

fn field_errors(error: &crate::error::VldError) -> Vec<FieldError> {
    crate::error::flatten_issues(error)
        .into_iter()
        .map(|(field, message)| FieldError { field, message })
        .collect()
}

fn ser_error(e: serde_json::Error) -> String {
    format!("Serialization error: {}", e)
}

/// Validate a serializable value against a vld schema type.
pub fn validate<S, T>(data: &T) -> Result<(), VldServerError>
where
    S: crate::schema::VldParse,
    T: serde::Serialize,
{
    let json = serde_json::to_value(data).map_err(|e| VldServerError::internal(ser_error(e)))?;
    validate_value::<S>(&json)
}

/// Validate a `serde_json::Value` against a vld schema type.
pub fn validate_value<S>(json: &serde_json::Value) -> Result<(), VldServerError>
where
    S: crate::schema::VldParse,
{
    S::vld_parse_value(json).map_err(VldServerError::from)?;
    Ok(())
}

/// Check a single value against a schema, returning the first error message if invalid.
///
/// A value that cannot be serialized is reported as an error, never as valid.
pub fn check_field<V, S>(value: &V, schema: &S) -> Option<String>
where
    V: serde::Serialize,
    S: crate::schema::VldSchema,
{
    check_field_all(value, schema).into_iter().next()
}

/// Check a single value, returning all error messages (empty if valid).
pub fn check_field_all<V, S>(value: &V, schema: &S) -> Vec<String>
where
    V: serde::Serialize,
    S: crate::schema::VldSchema,
{
    let json = match serde_json::to_value(value) {
        Ok(j) => j,
        Err(e) => return vec![ser_error(e)],
    };
    match schema.parse_value(&json) {
        Ok(_) => vec![],
        Err(e) => e.issues.iter().map(|i| i.message.clone()).collect(),
    }
}

/// Validate all fields of a serializable struct against a vld schema type.
///
/// Returns a list of [`FieldError`]s (empty if valid). A serialization failure
/// is reported as a single error with an empty `field`.
pub fn check_all_fields<S, T>(data: &T) -> Vec<FieldError>
where
    S: crate::schema::VldParse,
    T: serde::Serialize,
{
    let json = match serde_json::to_value(data) {
        Ok(j) => j,
        Err(e) => {
            return vec![FieldError {
                field: String::new(),
                message: ser_error(e),
            }];
        }
    };
    match S::vld_parse_value(&json) {
        Ok(_) => vec![],
        Err(e) => field_errors(&e),
    }
}

#[doc(hidden)]
pub fn __arg_errors<V, S>(name: &str, value: &V, schema: &S, out: &mut Vec<FieldError>)
where
    V: serde::Serialize,
    S: crate::schema::VldSchema,
{
    let json = match serde_json::to_value(value) {
        Ok(j) => j,
        Err(e) => {
            out.push(FieldError {
                field: name.to_string(),
                message: ser_error(e),
            });
            return;
        }
    };
    if let Err(e) = schema.parse_value(&json) {
        for issue in &e.issues {
            let sub = crate::error::format_path(&issue.path);
            let field = if sub.is_empty() {
                name.to_string()
            } else if sub.starts_with('[') {
                format!("{name}{sub}")
            } else {
                format!("{name}.{sub}")
            };
            out.push(FieldError {
                field,
                message: issue.message.clone(),
            });
        }
    }
}

/// Validate function arguments inline, accumulating all errors into a
/// `VldServerError`. Field names are the argument names (nested issues get
/// `arg.sub` paths).
#[doc(hidden)]
#[macro_export]
macro_rules! __form_validate_args {
    ($($field:ident => $schema:expr),* $(,)?) => {{
        let mut __vld_field_errors = ::std::vec::Vec::new();
        $(
            $crate::form::__arg_errors(stringify!($field), &$field, &($schema), &mut __vld_field_errors);
        )*
        if __vld_field_errors.is_empty() {
            ::std::result::Result::Ok::<(), $crate::form::VldServerError>(())
        } else {
            ::std::result::Result::Err($crate::form::VldServerError::validation(__vld_field_errors))
        }
    }};
}

#[doc(inline)]
pub use crate::__form_validate_args as validate_args;
