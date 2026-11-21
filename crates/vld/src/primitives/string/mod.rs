use serde_json::Value;

use crate::error::{ValidationIssue, VldError};
use crate::schema::VldSchema;

mod checks;
mod formats;

#[cfg(any(feature = "ts", feature = "fake"))]
pub(crate) use formats::{COUNTRIES_ALPHA2, COUNTRIES_ALPHA3, IBAN_LENGTHS};

#[cfg(feature = "openapi")]
use checks::regex_escape;
use checks::{Kind, PasswordRule, StringCheck};

#[cfg(feature = "string-advanced")]
mod typed;
#[cfg(feature = "string-advanced")]
pub use typed::{ZParsedString, ZUrl, ZUuid};

#[derive(Clone)]
enum StringTransform {
    Trim,
    ToLowerCase,
    ToUpperCase,
    /// Keep the first `n` characters.
    Truncate(usize),
    #[cfg(feature = "unicode")]
    Nfc,
    #[cfg(feature = "unicode")]
    Nfkc,
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

    /// Minimum UTF-8 length in **bytes** (`too_small`, origin `bytes`):
    /// for byte-limited storage. [`min`](Self::min) counts characters.
    pub fn min_bytes(self, n: usize) -> Self {
        self.push(Kind::MinBytes(n), None)
    }

    /// Maximum UTF-8 length in **bytes** (`too_big`, origin `bytes`).
    /// Not expressible in JSON Schema (`maxLength` counts characters), so it
    /// is left out of it.
    ///
    /// ```
    /// use vld::prelude::*;
    /// let name = vld::string().max_bytes(4);
    /// assert!(name.parse(r#""abcd""#).is_ok());
    /// assert!(name.parse(r#""абв""#).is_err()); // 3 characters, 6 bytes
    /// ```
    pub fn max_bytes(self, n: usize) -> Self {
        self.push(Kind::MaxBytes(n), None)
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

    /// Keep at most the first `n` characters (before the checks, in the
    /// order transforms were added): for storage limits that cut instead of
    /// rejecting.
    ///
    /// ```
    /// use vld::prelude::*;
    /// let title = vld::string().trim().truncate(5);
    /// assert_eq!(title.parse(r#"" Hello, world ""#).unwrap(), "Hello");
    /// ```
    pub fn truncate(mut self, n: usize) -> Self {
        self.transforms.push(StringTransform::Truncate(n));
        self
    }

    /// Convert to uppercase before validation.
    pub fn to_uppercase(mut self) -> Self {
        self.transforms.push(StringTransform::ToUpperCase);
        self
    }

    /// Normalize to Unicode NFC (canonical composition) before the checks:
    /// `e` + U+0301 becomes `é`, so equal-looking input compares and
    /// counts equal. Requires the `unicode` feature.
    ///
    /// ```
    /// use vld::prelude::*;
    /// let name = vld::string().normalize_nfc().max(4);
    /// assert_eq!(name.parse("\"Cafe\u{301}\"").unwrap(), "Caf\u{e9}");
    /// ```
    #[cfg(feature = "unicode")]
    #[cfg_attr(docsrs, doc(cfg(feature = "unicode")))]
    pub fn normalize_nfc(mut self) -> Self {
        self.transforms.push(StringTransform::Nfc);
        self
    }

    /// Normalize to Unicode NFKC (compatibility composition) before the
    /// checks: also folds full-width forms and ligatures (`ｆｉ` → `fi`,
    /// `ﬁ` → `fi`) — the form to compare identifiers / usernames in.
    /// Requires the `unicode` feature.
    #[cfg(feature = "unicode")]
    #[cfg_attr(docsrs, doc(cfg(feature = "unicode")))]
    pub fn normalize_nfkc(mut self) -> Self {
        self.transforms.push(StringTransform::Nfkc);
        self
    }

    /// Password rule: at least one lowercase letter (Unicode `Lowercase`).
    ///
    /// Every password rule reports its own issue
    /// (`invalid_string` / `password_lowercase`, `password_uppercase`,
    /// `password_digit`, `password_symbol`, `password_repeat`,
    /// `password_strength`), so a form can list every unmet rule. Combine
    /// with [`min`](Self::min) and [`redact`](crate::schema::VldSchema::redact)
    /// so the password never appears in errors:
    ///
    /// ```
    /// use vld::prelude::*;
    /// let password = vld::string()
    ///     .min(12)
    ///     .require_lowercase()
    ///     .require_uppercase()
    ///     .require_digit()
    ///     .require_symbol()
    ///     .max_repeat(2)
    ///     .redact();
    /// let err = password.parse(r#""aaapassword""#).unwrap_err();
    /// let keys: Vec<_> = err.issues.iter().map(|i| i.code.message_keys()[0].clone()).collect();
    /// assert_eq!(keys, [
    ///     "too_small.string",
    ///     "invalid_string.password_uppercase",
    ///     "invalid_string.password_digit",
    ///     "invalid_string.password_symbol",
    ///     "invalid_string.password_repeat",
    /// ]);
    /// assert!(!err.to_string().contains("aaapassword"));
    /// ```
    pub fn require_lowercase(self) -> Self {
        self.push(Kind::Password(PasswordRule::Lowercase), None)
    }

    /// [`require_lowercase`](Self::require_lowercase) with a custom message.
    pub fn require_lowercase_msg(self, msg: impl Into<String>) -> Self {
        self.push(Kind::Password(PasswordRule::Lowercase), Some(msg.into()))
    }

    /// Password rule: at least one uppercase letter (Unicode `Uppercase`).
    pub fn require_uppercase(self) -> Self {
        self.push(Kind::Password(PasswordRule::Uppercase), None)
    }

    /// [`require_uppercase`](Self::require_uppercase) with a custom message.
    pub fn require_uppercase_msg(self, msg: impl Into<String>) -> Self {
        self.push(Kind::Password(PasswordRule::Uppercase), Some(msg.into()))
    }

    /// Password rule: at least one ASCII digit.
    pub fn require_digit(self) -> Self {
        self.push(Kind::Password(PasswordRule::Digit), None)
    }

    /// [`require_digit`](Self::require_digit) with a custom message.
    pub fn require_digit_msg(self, msg: impl Into<String>) -> Self {
        self.push(Kind::Password(PasswordRule::Digit), Some(msg.into()))
    }

    /// Password rule: at least one symbol — a character that is neither
    /// alphanumeric nor whitespace (`!`, `_`, `€`, …).
    pub fn require_symbol(self) -> Self {
        self.push(Kind::Password(PasswordRule::Symbol), None)
    }

    /// [`require_symbol`](Self::require_symbol) with a custom message.
    pub fn require_symbol_msg(self, msg: impl Into<String>) -> Self {
        self.push(Kind::Password(PasswordRule::Symbol), Some(msg.into()))
    }

    /// Password rule: no character repeated more than `n` times in a row
    /// (`max_repeat(2)` rejects `aaa`).
    pub fn max_repeat(self, n: usize) -> Self {
        self.push(Kind::Password(PasswordRule::MaxRepeat(n)), None)
    }

    /// [`max_repeat`](Self::max_repeat) with a custom message.
    pub fn max_repeat_msg(self, n: usize, msg: impl Into<String>) -> Self {
        self.push(Kind::Password(PasswordRule::MaxRepeat(n)), Some(msg.into()))
    }

    /// Password rule: the [zxcvbn](https://docs.rs/zxcvbn) strength
    /// estimate is at least `min_score` (0 = too guessable … 4 = very
    /// unguessable; 3 is a common minimum). Requires the
    /// `password-strength` feature.
    #[cfg(feature = "password-strength")]
    #[cfg_attr(docsrs, doc(cfg(feature = "password-strength")))]
    pub fn password_strength(self, min_score: u8) -> Self {
        self.push(Kind::Password(PasswordRule::Strength(min_score)), None)
    }

    /// [`password_strength`](Self::password_strength) with a custom message.
    #[cfg(feature = "password-strength")]
    #[cfg_attr(docsrs, doc(cfg(feature = "password-strength")))]
    pub fn password_strength_msg(self, min_score: u8, msg: impl Into<String>) -> Self {
        self.push(
            Kind::Password(PasswordRule::Strength(min_score)),
            Some(msg.into()),
        )
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
                StringTransform::Truncate(n) => match s.char_indices().nth(*n) {
                    Some((at, _)) => s[..at].to_string(),
                    None => s,
                },
                #[cfg(feature = "unicode")]
                StringTransform::Nfc => {
                    unicode_normalization::UnicodeNormalization::nfc(s.as_str()).collect()
                }
                #[cfg(feature = "unicode")]
                StringTransform::Nfkc => {
                    unicode_normalization::UnicodeNormalization::nfkc(s.as_str()).collect()
                }
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
/// - Password rules add `format: "password"` and an `x-password` object
///   (`lowercase`, `uppercase`, `digit`, `symbol`: `true`; `maxRepeat`,
///   `minScore`: numbers).
#[cfg(feature = "openapi")]
impl crate::openapi::JsonSchema for ZString {
    fn json_schema(&self) -> serde_json::Value {
        let mut schema = serde_json::json!({"type": "string"});
        let (mut mins, mut maxs) = (vec![], vec![]);
        let mut formats: Vec<&str> = vec![];
        let mut patterns: Vec<String> = vec![];
        let mut extra = vec![];
        let mut password = serde_json::Map::new();
        for check in &self.checks {
            match &check.kind {
                Kind::Min(n) => mins.push(*n),
                Kind::Max(n) => maxs.push(*n),
                Kind::Len(n) => {
                    mins.push(*n);
                    maxs.push(*n);
                }
                Kind::NonEmpty => mins.push(1),
                // `maxLength` counts characters, not bytes.
                Kind::MinBytes(_) | Kind::MaxBytes(_) => {}
                Kind::StartsWith(p) => patterns.push(format!("^{}", regex_escape(p))),
                Kind::EndsWith(p) => patterns.push(format!("{}$", regex_escape(p))),
                Kind::Contains(p) => patterns.push(regex_escape(p)),
                #[cfg(feature = "regex")]
                Kind::Pattern(p) => patterns.push(p.as_str().to_string()),
                Kind::Rule(r) => extra.extend(r.json_schema()),
                #[cfg(feature = "crypto")]
                Kind::Crypto(c) => extra.push(c.json_schema()),
                Kind::Format(f) => {
                    if !formats.contains(&f.schema_format()) {
                        formats.push(f.schema_format());
                    }
                }
                Kind::Password(r) => {
                    if !formats.contains(&"password") {
                        formats.push("password");
                    }
                    let (name, value) = r.schema_entry();
                    password.insert(name.to_string(), value);
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
        // Password rules: OpenAPI's `format: password` plus the rules (not
        // expressible as ECMA patterns without look-arounds).
        if !password.is_empty() {
            schema["x-password"] = serde_json::Value::Object(password);
        }
        for keywords in extra {
            crate::rules::merge_keywords(&mut schema, keywords);
        }
        schema
    }
}

#[cfg(feature = "openapi")]
impl crate::openapi::CollectNestedSchemas for ZString {}
