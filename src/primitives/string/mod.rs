use serde_json::Value;

use crate::error::VldError;
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
}

impl ZString {
    pub fn new() -> Self {
        Self {
            checks: vec![],
            transforms: vec![],
            coerce: false,
            custom_type_error: None,
        }
    }

    fn push(mut self, kind: Kind, msg: impl Into<String>) -> Self {
        self.checks.push(StringCheck {
            kind,
            msg: msg.into(),
        });
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
        F: Fn(&str) -> Option<String>,
    {
        for check in &mut self.checks {
            if let Some(msg) = f(check.kind.key()) {
                check.msg = msg;
            }
        }
        self
    }

    /// Minimum string length in characters (inclusive).
    pub fn min(self, len: usize) -> Self {
        self.min_msg(len, format!("String must be at least {} characters", len))
    }

    /// Minimum string length with custom message.
    pub fn min_msg(self, len: usize, msg: impl Into<String>) -> Self {
        self.push(Kind::Min(len), msg)
    }

    /// Maximum string length in characters (inclusive).
    pub fn max(self, len: usize) -> Self {
        self.max_msg(len, format!("String must be at most {} characters", len))
    }

    /// Maximum string length with custom message.
    pub fn max_msg(self, len: usize, msg: impl Into<String>) -> Self {
        self.push(Kind::Max(len), msg)
    }

    /// Exact string length in characters.
    pub fn len(self, len: usize) -> Self {
        self.len_msg(len, format!("String must be exactly {} characters", len))
    }

    /// Exact string length with custom message.
    pub fn len_msg(self, len: usize, msg: impl Into<String>) -> Self {
        self.push(Kind::Len(len), msg)
    }

    /// Must match the given regex (emitted as JSON Schema `pattern`).
    ///
    /// Requires the `regex` feature.
    #[cfg(feature = "regex")]
    pub fn regex(self, re: regex_lite::Regex) -> Self {
        self.regex_msg(re, "String does not match pattern")
    }

    /// Must match the given regex, with custom message.
    ///
    /// Requires the `regex` feature.
    #[cfg(feature = "regex")]
    pub fn regex_msg(self, re: regex_lite::Regex, msg: impl Into<String>) -> Self {
        self.push(Kind::Regex(re), msg)
    }

    /// Must start with the given prefix.
    pub fn starts_with(self, prefix: impl Into<String>) -> Self {
        let p = prefix.into();
        let msg = format!("String must start with \"{}\"", p);
        self.starts_with_msg(p, msg)
    }

    /// Must start with the given prefix, with custom message.
    pub fn starts_with_msg(self, prefix: impl Into<String>, msg: impl Into<String>) -> Self {
        self.push(Kind::StartsWith(prefix.into()), msg)
    }

    /// Must end with the given suffix.
    pub fn ends_with(self, suffix: impl Into<String>) -> Self {
        let s = suffix.into();
        let msg = format!("String must end with \"{}\"", s);
        self.ends_with_msg(s, msg)
    }

    /// Must end with the given suffix, with custom message.
    pub fn ends_with_msg(self, suffix: impl Into<String>, msg: impl Into<String>) -> Self {
        self.push(Kind::EndsWith(suffix.into()), msg)
    }

    /// Must contain the given substring.
    pub fn contains(self, sub: impl Into<String>) -> Self {
        let s = sub.into();
        let msg = format!("String must contain \"{}\"", s);
        self.contains_msg(s, msg)
    }

    /// Must contain the given substring, with custom message.
    pub fn contains_msg(self, sub: impl Into<String>, msg: impl Into<String>) -> Self {
        self.push(Kind::Contains(sub.into()), msg)
    }

    /// Must not be empty.
    pub fn non_empty(self) -> Self {
        self.non_empty_msg("String must not be empty")
    }

    /// Must not be empty, with custom message.
    pub fn non_empty_msg(self, msg: impl Into<String>) -> Self {
        self.push(Kind::NonEmpty, msg)
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

    /// Generate a JSON Schema representation of this string schema.
    ///
    /// - Length bounds: the strictest `min`/`max`/`len`/`non_empty` wins.
    /// - Formats: the first goes into `format`, further ones into `allOf`.
    /// - `regex`/`starts_with`/`ends_with`/`contains` become `pattern`s
    ///   (first in `pattern`, further ones in `allOf`).
    ///
    /// Requires the `openapi` feature.
    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
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

impl Default for ZString {
    fn default() -> Self {
        Self::new()
    }
}

impl VldSchema for ZString {
    type Output = String;

    fn parse_value(&self, value: &Value) -> Result<String, VldError> {
        let type_err =
            || crate::primitives::type_error(&self.custom_type_error, "string", "string", value);

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

        let str_val = Value::String(s.clone());
        let mut errors = VldError::new();
        for check in &self.checks {
            if let Some(code) = check.kind.failure(&s) {
                errors.push_with_value(code, check.msg.clone(), &str_val);
            }
        }

        if errors.is_empty() {
            Ok(s)
        } else {
            Err(errors)
        }
    }
}
