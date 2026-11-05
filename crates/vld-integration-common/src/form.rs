//! Framework-agnostic server-function / form validation shared by
//! `vld-dioxus` and `vld-leptos`.

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
    /// use vld_integration_common::form::{FieldError, VldServerError};
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

impl From<vld::error::VldError> for VldServerError {
    fn from(error: vld::error::VldError) -> Self {
        Self::validation(field_errors(&error))
    }
}

fn field_errors(error: &vld::error::VldError) -> Vec<FieldError> {
    crate::flatten_issues(error)
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
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    let json = serde_json::to_value(data).map_err(|e| VldServerError::internal(ser_error(e)))?;
    validate_value::<S>(&json)
}

/// Validate a `serde_json::Value` against a vld schema type.
pub fn validate_value<S>(json: &serde_json::Value) -> Result<(), VldServerError>
where
    S: vld::schema::VldParse,
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
    S: vld::schema::VldSchema,
{
    check_field_all(value, schema).into_iter().next()
}

/// Check a single value, returning all error messages (empty if valid).
pub fn check_field_all<V, S>(value: &V, schema: &S) -> Vec<String>
where
    V: serde::Serialize,
    S: vld::schema::VldSchema,
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
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    let json = match serde_json::to_value(data) {
        Ok(j) => j,
        Err(e) => {
            return vec![FieldError {
                field: String::new(),
                message: ser_error(e),
            }]
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
    S: vld::schema::VldSchema,
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
            let sub = crate::format_path(&issue.path);
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
#[macro_export]
macro_rules! validate_args {
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

#[cfg(test)]
mod tests {
    use super::*;

    vld::schema! {
        #[derive(Debug)]
        struct Inner { email: String => vld::string().email() }
    }
    vld::schema! {
        #[derive(Debug)]
        struct Outer {
            name: String => vld::string().min(2),
            user: Inner => vld::nested(Inner::parse_value),
        }
    }

    #[test]
    fn paths_match_validate_args() {
        let err: VldServerError =
            Outer::parse_value(&serde_json::json!({"name": "A", "user": {"email": "bad"}}))
                .unwrap_err()
                .into();
        assert!(err.field_error("name").is_some());
        assert!(err.field_error("user.email").is_some());

        let name = "A".to_string();
        let err = crate::validate_args! { name => vld::string().min(2) }.unwrap_err();
        assert!(err.field_error("name").is_some());
    }

    #[test]
    fn error_fields_dedup_non_adjacent() {
        let f = |n: &str| FieldError {
            field: n.into(),
            message: "x".into(),
        };
        let e = VldServerError::validation(vec![f("a"), f("b"), f("a")]);
        assert_eq!(e.error_fields(), vec!["a", "b"]);
    }

    struct Bad;
    impl serde::Serialize for Bad {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("nope"))
        }
    }

    #[test]
    fn serialization_failure_is_an_error() {
        assert!(check_field(&Bad, &vld::string()).is_some());
        assert!(!check_field_all(&Bad, &vld::string()).is_empty());
        assert!(!check_all_fields::<Inner, _>(&Bad).is_empty());
    }
}
