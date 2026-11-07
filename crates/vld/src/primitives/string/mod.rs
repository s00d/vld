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
    /// `(true, false)` texts of coerced booleans; `None` = `"true"` / `"false"`.
    bool_text: Option<(String, String)>,
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
            bool_text: None,
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

    /// Must match a regular expression (emitted as JSON Schema `pattern`).
    ///
    /// Takes the source (`r"^[A-Z]{3}$"`, compiled here) or a prebuilt
    /// [`Pattern`](crate::rules::Pattern). Requires the `regex` feature.
    ///
    /// # Panics
    ///
    /// When the source is not a valid pattern (as `Regex::new(..).unwrap()`
    /// would); build a [`Pattern`](crate::rules::Pattern) with
    /// [`Pattern::new`](crate::rules::Pattern::new) to handle the error.
    ///
    /// ```
    /// use vld::prelude::*;
    /// let sku = vld::string().pattern(r"^[A-Z]{3}-\d{4}$");
    /// assert!(sku.parse(r#""ABC-1234""#).is_ok());
    /// ```
    #[cfg(feature = "regex")]
    #[cfg_attr(docsrs, doc(cfg(feature = "regex")))]
    pub fn pattern(self, pattern: impl crate::rules::IntoPattern) -> Self {
        self.push(Kind::Pattern(pattern.into_pattern()), None)
    }

    /// [`pattern`](Self::pattern) with a custom message.
    #[cfg(feature = "regex")]
    #[cfg_attr(docsrs, doc(cfg(feature = "regex")))]
    pub fn pattern_msg(
        self,
        pattern: impl crate::rules::IntoPattern,
        msg: impl Into<String>,
    ) -> Self {
        self.push(Kind::Pattern(pattern.into_pattern()), Some(msg.into()))
    }

    /// Check with a reusable [`Rule`](crate::rules::Rule) on the string
    /// (keeps the `ZString` builder; see [`vld::rules`](crate::rules)).
    pub fn rule(self, rule: impl crate::rules::Rule<str>) -> Self {
        self.push(Kind::Rule(std::sync::Arc::new(rule)), None)
    }

    /// [`rule`](Self::rule) with a custom message.
    pub fn rule_msg(self, rule: impl crate::rules::Rule<str>, msg: impl Into<String>) -> Self {
        self.push(Kind::Rule(std::sync::Arc::new(rule)), Some(msg.into()))
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

    /// Coerce like [`coerce`](Self::coerce), writing booleans as `on` / `off`
    /// instead of `"true"` / `"false"` (legacy stores often keep `"1"` / `"0"`).
    ///
    /// ```
    /// use vld::prelude::*;
    /// let flag = vld::string().coerce_bools("1", "0");
    /// assert_eq!(flag.parse("true").unwrap(), "1");
    /// assert_eq!(flag.parse("false").unwrap(), "0");
    /// assert_eq!(flag.parse("7").unwrap(), "7");
    /// ```
    pub fn coerce_bools(mut self, on: impl Into<String>, off: impl Into<String>) -> Self {
        self.coerce = true;
        self.bool_text = Some((on.into(), off.into()));
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
            Value::Bool(b) if self.coerce => match &self.bool_text {
                Some((on, off)) => if *b { on } else { off }.clone(),
                None => b.to_string(),
            },
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
                let msg = check
                    .msg
                    .clone()
                    .or_else(bulk)
                    .or_else(|| check.kind.rule_message());
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
/// - `pattern`/`starts_with`/`ends_with`/`contains` become `pattern`s
///   (first in `pattern`, further ones in `allOf`).
#[cfg(feature = "openapi")]
impl crate::openapi::JsonSchema for ZString {
    fn json_schema(&self) -> serde_json::Value {
        let mut schema = serde_json::json!({"type": "string"});
        let (mut mins, mut maxs) = (vec![], vec![]);
        let mut formats: Vec<&str> = vec![];
        let mut patterns: Vec<String> = vec![];
        let mut extra = vec![];
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
                Kind::Pattern(p) => patterns.push(p.as_str().to_string()),
                Kind::Rule(r) => extra.extend(r.json_schema()),
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
        for keywords in extra {
            crate::rules::merge_keywords(&mut schema, keywords);
        }
        schema
    }
}

#[cfg(feature = "openapi")]
impl crate::openapi::CollectNestedSchemas for ZString {}
