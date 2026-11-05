//! Validation errors: [`VldError`], [`ValidationIssue`], [`IssueCode`].

use std::fmt;

mod builder;
pub mod format;
pub mod i18n;

pub use crate::object::{FieldResult, ParseResult};
pub use builder::IssueBuilder;

/// A segment in a validation error path.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PathSegment {
    /// Object field name.
    Field(String),
    /// Array index.
    Index(usize),
}

impl fmt::Display for PathSegment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathSegment::Field(name) => write!(f, ".{}", name),
            PathSegment::Index(idx) => write!(f, "[{}]", idx),
        }
    }
}

/// Type of string validation that failed.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum StringValidation {
    Email,
    Url,
    UrlStrict,
    Uri,
    Uuid,
    UuidV1,
    UuidV4,
    UuidV7,
    Ip,
    Slug,
    Color,
    Currency,
    CountryCode,
    Locale,
    Cron,
    Regex,
    StartsWith,
    EndsWith,
    Ipv4,
    Ipv6,
    Cidr,
    Mac,
    Hex,
    CreditCard,
    Phone,
    PhoneE164,
    Semver,
    SemverFull,
    Jwt,
    Ascii,
    Alpha,
    Alphanumeric,
    Lowercase,
    Uppercase,
    Base64,
    IsoDate,
    IsoDatetime,
    IsoTime,
    Hostname,
    Cuid2,
    Ulid,
    Nanoid,
    Emoji,
}

/// Validation issue code — describes what went wrong.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum IssueCode {
    InvalidType { expected: String, received: String },
    TooSmall { minimum: f64, inclusive: bool },
    TooBig { maximum: f64, inclusive: bool },
    InvalidString { validation: StringValidation },
    NotInt,
    NotFinite,
    MissingField,
    UnrecognizedField,
    IoError,
    ParseError,
    Custom { code: String },
}

/// A single validation issue with path, message and received value.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ValidationIssue {
    pub code: IssueCode,
    pub message: String,
    pub path: Vec<PathSegment>,
    /// The value that was received (if available).
    pub received: Option<serde_json::Value>,
}

/// Collection of validation errors.
///
/// Errors are accumulated (not short-circuited), so all issues are reported at once.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VldError {
    pub issues: Vec<ValidationIssue>,
}

impl VldError {
    /// Create an empty error container.
    pub fn new() -> Self {
        Self { issues: vec![] }
    }

    /// Create an error with a single issue (no received value).
    pub fn single(code: IssueCode, message: impl Into<String>) -> Self {
        Self {
            issues: vec![ValidationIssue {
                code,
                message: message.into(),
                path: vec![],
                received: None,
            }],
        }
    }

    /// Create an error with a single issue and the received value.
    pub fn single_with_value(
        code: IssueCode,
        message: impl Into<String>,
        received: &serde_json::Value,
    ) -> Self {
        Self {
            issues: vec![ValidationIssue {
                code,
                message: message.into(),
                path: vec![],
                received: Some(truncate_value(received)),
            }],
        }
    }

    /// Prepend a path segment to all issues (used for nested objects/arrays).
    pub fn with_prefix(mut self, segment: PathSegment) -> Self {
        for issue in &mut self.issues {
            issue.path.insert(0, segment.clone());
        }
        self
    }

    /// Merge another error's issues into this one.
    pub fn merge(mut self, other: VldError) -> Self {
        self.issues.extend(other.issues);
        self
    }

    /// Check if there are no issues.
    pub fn is_empty(&self) -> bool {
        self.issues.is_empty()
    }

    /// Push a single issue (no received value).
    pub fn push(&mut self, code: IssueCode, message: impl Into<String>) {
        self.issues.push(ValidationIssue {
            code,
            message: message.into(),
            path: vec![],
            received: None,
        });
    }

    /// Push a single issue with the received value.
    pub fn push_with_value(
        &mut self,
        code: IssueCode,
        message: impl Into<String>,
        received: &serde_json::Value,
    ) {
        self.issues.push(ValidationIssue {
            code,
            message: message.into(),
            path: vec![],
            received: Some(truncate_value(received)),
        });
    }
}

impl Default for VldError {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for VldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, issue) in self.issues.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            if !issue.path.is_empty() {
                let path_str: String = issue.path.iter().map(|p| p.to_string()).collect();
                write!(f, "{}: ", path_str)?;
            }
            write!(f, "{}", issue.message)?;
            if let Some(val) = &issue.received {
                write!(f, ", received {}", format_value_short(val))?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for VldError {}

impl IssueCode {
    /// Stable string key for this error code. Useful for i18n and error mapping.
    pub fn key(&self) -> &str {
        match self {
            IssueCode::InvalidType { .. } => "invalid_type",
            IssueCode::TooSmall { .. } => "too_small",
            IssueCode::TooBig { .. } => "too_big",
            IssueCode::InvalidString { .. } => "invalid_string",
            IssueCode::NotInt => "not_int",
            IssueCode::NotFinite => "not_finite",
            IssueCode::MissingField => "missing_field",
            IssueCode::UnrecognizedField => "unrecognized_field",
            IssueCode::IoError => "io_error",
            IssueCode::ParseError => "parse_error",
            IssueCode::Custom { code } => code,
        }
    }

    /// Extract key-value parameters from this error code for message formatting.
    ///
    /// Useful for i18n: format templates like `"Must be at least {minimum}"`.
    pub fn params(&self) -> Vec<(&str, String)> {
        match self {
            IssueCode::InvalidType { expected, received } => {
                vec![
                    ("expected", expected.clone()),
                    ("received", received.clone()),
                ]
            }
            IssueCode::TooSmall { minimum, inclusive } => {
                vec![
                    ("minimum", minimum.to_string()),
                    ("inclusive", inclusive.to_string()),
                ]
            }
            IssueCode::TooBig { maximum, inclusive } => {
                vec![
                    ("maximum", maximum.to_string()),
                    ("inclusive", inclusive.to_string()),
                ]
            }
            IssueCode::InvalidString { validation } => {
                vec![("validation", format!("{:?}", validation))]
            }
            _ => vec![],
        }
    }
}

/// Returns the JSON type name for a value.
pub(crate) fn value_type_name(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
    .to_string()
}

/// Return the value as a JSON object, or an `invalid_type` error
/// ("Expected object, received ...").
///
/// Shared by [`ZObject`](crate::object::ZObject), `schema!` and `#[derive(Validate)]`.
pub(crate) fn expect_object(
    value: &serde_json::Value,
) -> Result<&serde_json::Map<String, serde_json::Value>, VldError> {
    value.as_object().ok_or_else(|| {
        let received = value_type_name(value);
        VldError::single(
            IssueCode::InvalidType {
                expected: "object".to_string(),
                received: received.clone(),
            },
            format!("Expected object, received {}", received),
        )
    })
}

/// Format a JSON value for display in errors (short form).
pub(crate) fn format_value_short(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => {
            let char_count = s.chars().count();
            if char_count > 50 {
                let prefix: String = s.chars().take(47).collect();
                format!("\"{}...\"", prefix)
            } else {
                format!("\"{}\"", s)
            }
        }
        serde_json::Value::Array(arr) => format!("Array(len={})", arr.len()),
        serde_json::Value::Object(obj) => format!("Object(keys={})", obj.len()),
    }
}

/// Truncate large values to avoid storing huge payloads in errors.
pub(crate) fn truncate_value(value: &serde_json::Value) -> serde_json::Value {
    match value {
        // Slice by chars, not bytes: a byte slice can split a multi-byte
        // UTF-8 sequence and panic on attacker-controlled input.
        serde_json::Value::String(s) if s.chars().count() > 100 => {
            let prefix: String = s.chars().take(97).collect();
            serde_json::Value::String(format!("{}...", prefix))
        }
        serde_json::Value::Array(arr) if arr.len() > 5 => {
            let mut truncated: Vec<serde_json::Value> = arr[..5].to_vec();
            truncated.push(serde_json::Value::String(format!(
                "... ({} more)",
                arr.len() - 5
            )));
            serde_json::Value::Array(truncated)
        }
        _ => value.clone(),
    }
}
