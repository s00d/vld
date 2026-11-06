use serde_json::Value;

use crate::error::{ValidationIssue, VldError};
use crate::schema::VldSchema;

mod checks;
mod formats;

#[cfg(feature = "openapi")]
use checks::regex_escape;
use checks::{Kind, StringCheck};

#[derive(Clone)]
enum StringTransform {
    Trim,
    ToLowerCase,
    ToUpperCase,
}

/// Schema for string validation. Created via [`vld::string()`](crate::string).
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// let schema = vld::string().min(3).max(20).email();
/// ```
#[derive(Clone)]
pub struct ZString {
    checks: Vec<StringCheck>,
    transforms: Vec<StringTransform>,
    coerce: bool,
    custom_type_error: Option<String>,
    messages: crate::primitives::Messages,
}

impl ZString {
    /// Create the schema (the same as its `vld::` constructor function).
    pub fn new() -> Self {
        Self {
            checks: vec![],
            transforms: vec![],
            coerce: false,
            custom_type_error: None,
            messages: None,
        }
    }

    fn push(mut self, kind: Kind, msg: Option<String>) -> Self {
        self.checks.push(StringCheck { kind, msg });
        self
    }

    /// Set a custom error message for type mismatch (when the input is not a string).
    ///
    /// # Example
    /// ```
    /// use vld::prelude::*;
    /// let schema = vld::string().type_error("Must be text!");
    /// let err = schema.parse("42").unwrap_err();
    /// assert!(err.issues[0].message.contains("Must be text!"));
    /// ```
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    /// Override error messages in bulk by check key.
    ///
    /// The closure receives the check key (e.g. `"too_small"`, `"invalid_email"`)
    /// and should return `Some(new_message)` to replace, or `None` to keep the original.
    ///
    /// Keys: `"too_small"`, `"too_big"`, `"invalid_length"`, `"non_empty"`,
    /// `"invalid_regex"`, `"invalid_starts_with"`, `"invalid_ends_with"`,
    /// `"invalid_contains"`, and `"invalid_<format>"` for every format check
    /// (`"invalid_email"`, `"invalid_url"`, `"invalid_uuid"`, `"invalid_ip"`,
    /// `"invalid_currency"`, `"invalid_phone_e164"`, …).
    ///
    /// The closure is consulted when an issue is built, so it covers checks
    /// added before **and after** this call; a per-check message
    /// (`*_msg(...)`) takes precedence over it. A later call replaces it.
    ///
    /// # Example
    /// ```
    /// use vld::prelude::*;
    /// let schema = vld::string().min(3).email()
    ///     .with_messages(|key| match key {
    ///         "too_small" => Some("Too short!".into()),
    ///         "invalid_email" => Some("Bad email!".into()),
    ///         _ => None,
    ///     });
    /// ```
    pub fn with_messages<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> Option<String> + Send + Sync + 'static,
    {
        self.messages = Some(std::sync::Arc::new(f));
        self
    }

    /// Minimum string length in characters (inclusive).
    pub fn min(self, len: usize) -> Self {
        self.push(Kind::Min(len), None)
    }

    /// Minimum string length with custom message.
    pub fn min_msg(self, len: usize, msg: impl Into<String>) -> Self {
        self.push(Kind::Min(len), Some(msg.into()))
    }

    /// Maximum string length in characters (inclusive).
    pub fn max(self, len: usize) -> Self {
        self.push(Kind::Max(len), None)
    }

    /// Maximum string length with custom message.
    pub fn max_msg(self, len: usize, msg: impl Into<String>) -> Self {
        self.push(Kind::Max(len), Some(msg.into()))
    }

    /// Exact string length in characters.
    pub fn len(self, len: usize) -> Self {
        self.push(Kind::Len(len), None)
    }

    /// Exact string length with custom message.
    pub fn len_msg(self, len: usize, msg: impl Into<String>) -> Self {
        self.push(Kind::Len(len), Some(msg.into()))
    }

    /// Must match the given regex (emitted as JSON Schema `pattern`).
    ///
    /// Requires the `regex` feature.
    #[cfg(feature = "regex")]
    #[cfg_attr(docsrs, doc(cfg(feature = "regex")))]
    pub fn regex(self, re: regex_lite::Regex) -> Self {
        self.push(Kind::Regex(re), None)
    }

    /// Must match the given regex, with custom message.
    ///
    /// Requires the `regex` feature.
    #[cfg(feature = "regex")]
    #[cfg_attr(docsrs, doc(cfg(feature = "regex")))]
    pub fn regex_msg(self, re: regex_lite::Regex, msg: impl Into<String>) -> Self {
        self.push(Kind::Regex(re), Some(msg.into()))
    }

    /// Must start with the given prefix.
    pub fn starts_with(self, prefix: impl Into<String>) -> Self {
        self.push(Kind::StartsWith(prefix.into()), None)
    }

    /// Must start with the given prefix, with custom message.
    pub fn starts_with_msg(self, prefix: impl Into<String>, msg: impl Into<String>) -> Self {
        self.push(Kind::StartsWith(prefix.into()), Some(msg.into()))
    }

    /// Must end with the given suffix.
    pub fn ends_with(self, suffix: impl Into<String>) -> Self {
        self.push(Kind::EndsWith(suffix.into()), None)
    }

    /// Must end with the given suffix, with custom message.
    pub fn ends_with_msg(self, suffix: impl Into<String>, msg: impl Into<String>) -> Self {
        self.push(Kind::EndsWith(suffix.into()), Some(msg.into()))
    }

    /// Must contain the given substring.
    pub fn contains(self, sub: impl Into<String>) -> Self {
        self.push(Kind::Contains(sub.into()), None)
    }

    /// Must contain the given substring, with custom message.
    pub fn contains_msg(self, sub: impl Into<String>, msg: impl Into<String>) -> Self {
        self.push(Kind::Contains(sub.into()), Some(msg.into()))
    }

    /// Must not be empty.
    pub fn non_empty(self) -> Self {
        self.push(Kind::NonEmpty, None)
    }

    /// Must not be empty, with custom message.
    pub fn non_empty_msg(self, msg: impl Into<String>) -> Self {
        self.push(Kind::NonEmpty, Some(msg.into()))
    }

    /// Trim whitespace before validation.
    pub fn trim(mut self) -> Self {
        self.transforms.push(StringTransform::Trim);
        self
    }

    /// Convert to lowercase before validation.
    pub fn to_lowercase(mut self) -> Self {
        self.transforms.push(StringTransform::ToLowerCase);
        self
    }

    /// Convert to uppercase before validation.
    pub fn to_uppercase(mut self) -> Self {
        self.transforms.push(StringTransform::ToUpperCase);
        self
    }

    /// Coerce non-string values (numbers, booleans) to string.
    pub fn coerce(mut self) -> Self {
        self.coerce = true;
        self
    }
}

impl Default for ZString {
    fn default() -> Self {
        Self::new()
    }
}

impl VldSchema for ZString {
    type Output = String;

    fn parse_value(&self, value: &Value) -> Result<String, VldError> {
        let type_err = || crate::primitives::type_error(&self.custom_type_error, "string", value);

        let mut s = match value {
            Value::String(s) => s.clone(),
            Value::Number(n) if self.coerce => n.to_string(),
            Value::Bool(b) if self.coerce => b.to_string(),
            _ => return Err(type_err()),
        };

        for t in &self.transforms {
            s = match t {
                StringTransform::Trim => s.trim().to_string(),
                StringTransform::ToLowerCase => s.to_lowercase(),
                StringTransform::ToUpperCase => s.to_uppercase(),
            };
        }

        // The (transformed) string is reported as the received value; built
        // only once something fails.
        let mut received: Option<Value> = None;
        let mut errors = VldError::new();
        for check in &self.checks {
            if let Some(code) = check.kind.failure(&s) {
                let received = received.get_or_insert_with(|| Value::String(s.clone()));
                let bulk = || self.messages.as_ref().and_then(|f| f(check.kind.key()));
                let msg = check.msg.clone().or_else(bulk);
                errors.issues.push(
                    ValidationIssue::with_override(code, msg.as_ref()).with_received(received),
                );
            }
        }

        if errors.is_empty() {
            Ok(s)
        } else {
            Err(errors)
        }
    }
}

/// Generate a JSON Schema representation of this string schema.
///
/// - Length bounds: the strictest `min`/`max`/`len`/`non_empty` wins.
/// - Formats: the first goes into `format`, further ones into `allOf`.
/// - `regex`/`starts_with`/`ends_with`/`contains` become `pattern`s
///   (first in `pattern`, further ones in `allOf`).
#[cfg(feature = "openapi")]
impl crate::openapi::JsonSchema for ZString {
    fn json_schema(&self) -> serde_json::Value {
        let mut schema = serde_json::json!({"type": "string"});
        let (mut mins, mut maxs) = (vec![], vec![]);
        let mut formats: Vec<&str> = vec![];
        let mut patterns: Vec<String> = vec![];
        for check in &self.checks {
            match &check.kind {
                Kind::Min(n) => mins.push(*n),
                Kind::Max(n) => maxs.push(*n),
                Kind::Len(n) => {
                    mins.push(*n);
                    maxs.push(*n);
                }
                Kind::NonEmpty => mins.push(1),
                Kind::StartsWith(p) => patterns.push(format!("^{}", regex_escape(p))),
                Kind::EndsWith(p) => patterns.push(format!("{}$", regex_escape(p))),
                Kind::Contains(p) => patterns.push(regex_escape(p)),
                #[cfg(feature = "regex")]
                Kind::Regex(re) => patterns.push(re.as_str().to_string()),
                Kind::Format(f) => {
                    if !formats.contains(&f.schema_format()) {
                        formats.push(f.schema_format());
                    }
                }
            }
        }
        crate::primitives::length_schema(&mut schema, ("minLength", "maxLength"), mins, maxs);
        let mut all_of = vec![];
        let mut formats = formats.into_iter();
        if let Some(f) = formats.next() {
            schema["format"] = serde_json::json!(f);
        }
        all_of.extend(formats.map(|f| serde_json::json!({ "format": f })));
        let mut patterns = patterns.into_iter();
        if let Some(p) = patterns.next() {
            schema["pattern"] = serde_json::json!(p);
        }
        all_of.extend(patterns.map(|p| serde_json::json!({ "pattern": p })));
        if !all_of.is_empty() {
            schema["allOf"] = serde_json::Value::Array(all_of);
        }
        schema
    }
}

#[cfg(feature = "openapi")]
impl crate::openapi::CollectNestedSchemas for ZString {}
