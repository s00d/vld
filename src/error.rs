use std::fmt;

/// A segment in a validation error path.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "deserialize", derive(serde::Deserialize))]
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
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "deserialize", derive(serde::Deserialize))]
pub enum StringValidation {
    Email,
    Url,
    Uuid,
    Regex,
    StartsWith,
    EndsWith,
    Ipv4,
    Ipv6,
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
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "deserialize", derive(serde::Deserialize))]
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
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "deserialize", derive(serde::Deserialize))]
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
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "deserialize", derive(serde::Deserialize))]
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

/// Fluent builder for constructing a single [`ValidationIssue`].
///
/// Obtained via [`VldError::issue()`]. Push it into a `VldError` with
/// [`.finish()`](IssueBuilder::finish).
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// let mut errors = VldError::new();
/// errors
///     .issue(IssueCode::Custom { code: "password_weak".into() })
///     .message("Password is too weak")
///     .path_field("password")
///     .received(&serde_json::json!("123"))
///     .finish();
/// assert_eq!(errors.issues.len(), 1);
/// assert_eq!(errors.issues[0].message, "Password is too weak");
/// ```
pub struct IssueBuilder<'a> {
    errors: &'a mut VldError,
    code: IssueCode,
    message: Option<String>,
    path: Vec<PathSegment>,
    received: Option<serde_json::Value>,
}

impl<'a> IssueBuilder<'a> {
    /// Set the error message.
    pub fn message(mut self, msg: impl Into<String>) -> Self {
        self.message = Some(msg.into());
        self
    }

    /// Add a field path segment.
    pub fn path_field(mut self, name: impl Into<String>) -> Self {
        self.path.push(PathSegment::Field(name.into()));
        self
    }

    /// Add an index path segment.
    pub fn path_index(mut self, idx: usize) -> Self {
        self.path.push(PathSegment::Index(idx));
        self
    }

    /// Attach the received value.
    pub fn received(mut self, value: &serde_json::Value) -> Self {
        self.received = Some(truncate_value(value));
        self
    }

    /// Finish building and push the issue into the parent `VldError`.
    pub fn finish(self) {
        let msg = self
            .message
            .unwrap_or_else(|| format!("Validation error: {}", self.code.key()));
        self.errors.issues.push(ValidationIssue {
            code: self.code,
            message: msg,
            path: self.path,
            received: self.received,
        });
    }
}

impl VldError {
    /// Start building a new issue with the fluent API.
    ///
    /// Call `.message(...)`, `.path_field(...)`, `.received(...)` etc., then
    /// `.finish()` to push the issue.
    ///
    /// # Example
    /// ```
    /// use vld::prelude::*;
    ///
    /// let mut errors = VldError::new();
    /// errors
    ///     .issue(IssueCode::Custom { code: "my_check".into() })
    ///     .message("something went wrong")
    ///     .path_field("field_name")
    ///     .finish();
    /// ```
    pub fn issue(&mut self, code: IssueCode) -> IssueBuilder<'_> {
        IssueBuilder {
            errors: self,
            code,
            message: None,
            path: vec![],
            received: None,
        }
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
#[doc(hidden)]
pub fn value_type_name(value: &serde_json::Value) -> String {
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

/// Format a JSON value for display in errors (short form).
pub fn format_value_short(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => {
            if s.len() > 50 {
                format!("\"{}...\"", &s[..47])
            } else {
                format!("\"{}\"", s)
            }
        }
        serde_json::Value::Array(arr) => format!("Array(len={})", arr.len()),
        serde_json::Value::Object(obj) => format!("Object(keys={})", obj.len()),
    }
}

/// Result of validating a single field.
///
/// Returned by `validate_fields()` generated by the [`schema!`](crate::schema!) macro.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "deserialize", derive(serde::Deserialize))]
pub struct FieldResult {
    /// Field name.
    pub name: String,
    /// Raw input value (before validation).
    pub input: serde_json::Value,
    /// `Ok(json_value)` if the field is valid (output serialized to JSON),
    /// `Err(error)` if validation failed.
    pub result: Result<serde_json::Value, VldError>,
}

impl FieldResult {
    /// Whether this field passed validation.
    pub fn is_ok(&self) -> bool {
        self.result.is_ok()
    }

    /// Whether this field failed validation.
    pub fn is_err(&self) -> bool {
        self.result.is_err()
    }
}

impl std::fmt::Display for FieldResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.result {
            Ok(v) => write!(f, "✔ {}: {}", self.name, format_value_short(v)),
            Err(e) => {
                let received = format_value_short(&self.input);
                for (i, issue) in e.issues.iter().enumerate() {
                    if i > 0 {
                        writeln!(f)?;
                    }
                    write!(
                        f,
                        "✖ {}: {} (received: {})",
                        self.name, issue.message, received
                    )?;
                }
                Ok(())
            }
        }
    }
}

/// Result of lenient parsing: contains the (possibly partial) struct and
/// per-field diagnostics.
///
/// Created by `parse_lenient()` / `parse_lenient_value()` generated by
/// [`impl_validate_fields!`](crate::impl_validate_fields).
///
/// The struct is always constructed — invalid fields fall back to
/// `Default::default()`. You can inspect which fields passed/failed via
/// [`fields()`](Self::fields), and save the result to a file at any time
/// via `save_to_file()` (requires the `serialize` feature).
#[derive(Debug)]
pub struct ParseResult<T> {
    /// The constructed struct (invalid fields use `Default`).
    pub value: T,
    /// Per-field validation diagnostics.
    field_results: Vec<FieldResult>,
}

impl<T> ParseResult<T> {
    /// Create a new parse result.
    pub fn new(value: T, field_results: Vec<FieldResult>) -> Self {
        Self {
            value,
            field_results,
        }
    }

    /// All per-field results.
    pub fn fields(&self) -> &[FieldResult] {
        &self.field_results
    }

    /// Get a specific field's result by name.
    ///
    /// Returns `None` if no field with that name exists.
    pub fn field(&self, name: &str) -> Option<&FieldResult> {
        self.field_results.iter().find(|f| f.name == name)
    }

    /// Only the fields that passed validation.
    pub fn valid_fields(&self) -> Vec<&FieldResult> {
        self.field_results.iter().filter(|f| f.is_ok()).collect()
    }

    /// Only the fields that failed validation.
    pub fn error_fields(&self) -> Vec<&FieldResult> {
        self.field_results.iter().filter(|f| f.is_err()).collect()
    }

    /// Whether all fields passed validation.
    pub fn is_valid(&self) -> bool {
        self.field_results.iter().all(|f| f.is_ok())
    }

    /// Whether at least one field failed.
    pub fn has_errors(&self) -> bool {
        self.field_results.iter().any(|f| f.is_err())
    }

    /// Number of valid fields.
    pub fn valid_count(&self) -> usize {
        self.field_results.iter().filter(|f| f.is_ok()).count()
    }

    /// Number of invalid fields.
    pub fn error_count(&self) -> usize {
        self.field_results.iter().filter(|f| f.is_err()).count()
    }

    /// Consume and return the inner struct.
    pub fn into_value(self) -> T {
        self.value
    }

    /// Consume and return both the struct and field results.
    pub fn into_parts(self) -> (T, Vec<FieldResult>) {
        (self.value, self.field_results)
    }
}

#[cfg(feature = "serialize")]
impl<T: serde::Serialize> ParseResult<T> {
    /// Serialize the struct to a JSON file.
    ///
    /// Requires the `serialize` and `std` features.
    #[cfg(feature = "std")]
    pub fn save_to_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(&self.value)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json)
    }

    /// Serialize the struct to a JSON string.
    ///
    /// Requires the `serialize` feature.
    pub fn to_json_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.value)
    }

    /// Serialize the struct to a `serde_json::Value`.
    ///
    /// Requires the `serialize` feature.
    pub fn to_json_value(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(&self.value)
    }
}

impl<T: std::fmt::Debug> std::fmt::Display for ParseResult<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "ParseResult ({} valid, {} errors):",
            self.valid_count(),
            self.error_count()
        )?;
        for field in &self.field_results {
            writeln!(f, "  {}", field)?;
        }
        Ok(())
    }
}

/// Truncate large values to avoid storing huge payloads in errors.
fn truncate_value(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) if s.len() > 100 => {
            serde_json::Value::String(format!("{}...", &s[..97]))
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
