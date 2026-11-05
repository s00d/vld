//! Validation errors: [`VldError`], [`ValidationIssue`], [`IssueCode`].
//!
//! Every issue carries a machine-readable [`IssueCode`] and a `message`. The
//! default message is rendered from the English template table
//! ([`i18n::english()`]) using [`IssueCode::message_keys()`] and
//! [`IssueCode::params()`]; per-check overrides (`*_msg` builders,
//! `.message()`, `type_error()`) replace it. [`i18n::translate_error`]
//! re-renders messages with another table.
//!
//! # Issue codes
//!
//! | Variant | `key()` | Template keys (specific → generic) | Params |
//! |---|---|---|---|
//! | `InvalidType` | `invalid_type` | `invalid_type` | `expected`, `received` |
//! | `TooSmall` | `too_small` | `too_small.<origin>`, `too_small` (exclusive: `too_small_exclusive[.<origin>]`) | `origin`, `minimum`, `inclusive` |
//! | `TooBig` | `too_big` | `too_big.<origin>`, `too_big` (exclusive: `too_big_exclusive[.<origin>]`) | `origin`, `maximum`, `inclusive` |
//! | `InvalidLength` | `invalid_length` | `invalid_length.<origin>` | `origin`, `expected` |
//! | `InvalidString` | `invalid_string` | `invalid_string.<validation>` | `validation` (+ `prefix` / `suffix` / `substring`) |
//! | `InvalidFormat` | `invalid_format` | `invalid_format.<format>` | `format` |
//! | `InvalidDate` | `invalid_date` | `invalid_date.<kind>` | `kind` |
//! | `InvalidTimezoneOffset` | `invalid_timezone_offset` | — | `expected` |
//! | `NotInPast` | `not_in_past` | `not_in_past.<kind>` | `kind` |
//! | `NotInFuture` | `not_in_future` | `not_in_future.<kind>` | `kind` |
//! | `NotInt` | `not_int` | — | — |
//! | `NotFinite` | `not_finite` | — | — |
//! | `NotMultipleOf` | `not_multiple_of` | — | `divisor` |
//! | `NotSafeInteger` | `not_safe_integer` | — | — |
//! | `InvalidLiteral` | `invalid_literal` | — | `expected` |
//! | `InvalidEnumValue` | `invalid_enum_value` | — | `options` |
//! | `InvalidUnion` | `invalid_union` | — | — |
//! | `InvalidDiscriminator` | `invalid_discriminator` | — | `options` |
//! | `NotUnique` | `not_unique` | — | — |
//! | `MissingContains` | `missing_contains` | — | — |
//! | `DuplicateKey` | `duplicate_key` | — | — |
//! | `InvalidMapEntry` | `invalid_map_entry` | — | — |
//! | `MissingField` | `missing_field` | — | — |
//! | `UnrecognizedField` | `unrecognized_field` | — | — |
//! | `PathOutsideBase` | `path_outside_base` | — | `base` |
//! | `PathNotAbsolute` | `path_not_absolute` | — | — |
//! | `PathNotRelative` | `path_not_relative` | — | — |
//! | `JsonTooDeep` | `json_too_deep` | — | `max_depth` |
//! | `FileTypeNotAllowed` | `file_type_not_allowed` | — | `kind`, `actual`, `allowed` |
//! | `FileTypeDenied` | `file_type_denied` | — | `kind`, `actual` |
//! | `FileTypeUnknown` | `file_type_unknown` | — | `kind` |
//! | `ChecksumMismatch` | `checksum_mismatch` | — | `algorithm` |
//! | `MissingExif` | `missing_exif` | — | — |
//! | `SerializeError` | `serialize_error` | — | `detail` |
//! | `IoError` | `io_error` | — | — |
//! | `ParseError` | `parse_error` | — | — |
//! | `Custom` | the user code | `<code>`, `custom` | `code` |
//!
//! Origins ([`SizeOrigin::key`]): `string`, `number`, `decimal`, `array`,
//! `tuple`, `set`, `record`, `object`, `bytes`, `date`, `datetime`,
//! `duration`, `file`, `image_width`, `image_height`, `port`, `contains`.

use std::fmt;

mod builder;
mod code;
pub mod format;
pub mod i18n;
mod i18n_langs;

pub use crate::object::{FieldResult, ParseResult};
pub use builder::IssueBuilder;
pub use code::{DateKind, FileTypeKind, FormatKind, IssueCode, SizeOrigin, StringValidation};

/// A segment in a validation error path.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/// A single validation issue with path, message and received value.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ValidationIssue {
    pub code: IssueCode,
    pub message: String,
    pub path: Vec<PathSegment>,
    /// The value that was received (if available).
    pub received: Option<serde_json::Value>,
}

impl ValidationIssue {
    /// Issue with the default English message for `code`, no path, no value.
    pub fn new(code: IssueCode) -> Self {
        Self {
            message: code.default_message(),
            code,
            path: vec![],
            received: None,
        }
    }

    /// Replace the message.
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// Attach the received value (large strings / arrays are truncated).
    pub fn with_received(mut self, value: &serde_json::Value) -> Self {
        self.received = Some(truncate_value(value));
        self
    }
}

impl From<ValidationIssue> for VldError {
    fn from(issue: ValidationIssue) -> Self {
        Self {
            issues: vec![issue],
        }
    }
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

    /// Error with a single issue and its default English message.
    pub fn from_code(code: IssueCode) -> Self {
        ValidationIssue::new(code).into()
    }

    /// Error with a single issue, its default English message and the received value.
    pub fn from_code_with_value(code: IssueCode, received: &serde_json::Value) -> Self {
        ValidationIssue::new(code).with_received(received).into()
    }

    /// Create an error with a single issue (no received value).
    pub fn single(code: IssueCode, message: impl Into<String>) -> Self {
        ValidationIssue::new_raw(code, message.into()).into()
    }

    /// Create an error with a single issue and the received value.
    pub fn single_with_value(
        code: IssueCode,
        message: impl Into<String>,
        received: &serde_json::Value,
    ) -> Self {
        ValidationIssue::new_raw(code, message.into())
            .with_received(received)
            .into()
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
        self.issues
            .push(ValidationIssue::new_raw(code, message.into()));
    }

    /// Push a single issue with the received value.
    pub fn push_with_value(
        &mut self,
        code: IssueCode,
        message: impl Into<String>,
        received: &serde_json::Value,
    ) {
        self.issues
            .push(ValidationIssue::new_raw(code, message.into()).with_received(received));
    }

    /// Push a single issue with its default English message.
    pub fn push_code(&mut self, code: IssueCode) {
        self.issues.push(ValidationIssue::new(code));
    }

    /// Push a single issue with its default English message and the received value.
    pub fn push_code_with_value(&mut self, code: IssueCode, received: &serde_json::Value) {
        self.issues
            .push(ValidationIssue::new(code).with_received(received));
    }
}

impl ValidationIssue {
    fn new_raw(code: IssueCode, message: String) -> Self {
        Self {
            code,
            message,
            path: vec![],
            received: None,
        }
    }

    /// Issue with `custom` as message when set, else the default message.
    pub(crate) fn with_override(code: IssueCode, custom: Option<&String>) -> Self {
        match custom {
            Some(msg) => Self::new_raw(code, msg.clone()),
            None => Self::new(code),
        }
    }
}

/// Error with one issue: `custom` message (or the default) plus the received value.
pub(crate) fn issue_error(
    code: IssueCode,
    custom: Option<&String>,
    received: &serde_json::Value,
) -> VldError {
    ValidationIssue::with_override(code, custom)
        .with_received(received)
        .into()
}

/// Error of a field whose key is absent from its object.
///
/// A required field rejects the implied `null` with `invalid_type`
/// ("received null"); for an absent key that is reported as a single
/// [`IssueCode::MissingField`] instead. A custom type-error message is kept.
pub(crate) fn absent_field_error(err: VldError) -> VldError {
    let rejected_null = err.issues.iter().find(|i| {
        i.path.is_empty()
            && matches!(&i.code, IssueCode::InvalidType { received, .. } if received == "null")
    });
    match rejected_null {
        Some(issue) => {
            let custom = issue.message != issue.code.default_message();
            let missing = ValidationIssue::new(IssueCode::MissingField);
            if custom {
                missing.with_message(issue.message.clone()).into()
            } else {
                missing.into()
            }
        }
        None => err,
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
            // `invalid_type` messages already name the received type.
            if let (Some(val), false) = (
                &issue.received,
                matches!(issue.code, IssueCode::InvalidType { .. }),
            ) {
                write!(f, ", received {}", format_value_short(val))?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for VldError {}

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
        VldError::from_code(IssueCode::InvalidType {
            expected: "object".to_string(),
            received: value_type_name(value),
        })
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
