//! Reusable validation rules.
//!
//! A [`Rule`] is a named check you write once and attach anywhere: on a
//! string schema with [`ZString::rule`](crate::primitives::ZString::rule), or on
//! any schema with [`VldSchema::rule`](crate::schema::VldSchema::rule). A
//! failing rule reports `IssueCode::Custom { code: <key> }`, so its message can
//! be translated by key (see [`i18n`](crate::i18n)) and set per schema with
//! `with_messages` / `rule_msg`.
//!
//! ```
//! use vld::prelude::*;
//! use vld::rules::Rule;
//!
//! /// Russian mobile number: `+7` and ten digits.
//! struct RuPhone;
//!
//! impl Rule<str> for RuPhone {
//!     fn key(&self) -> &str {
//!         "ru_phone"
//!     }
//!     fn check(&self, s: &str) -> bool {
//!         s.strip_prefix("+7")
//!             .is_some_and(|d| d.len() == 10 && d.bytes().all(|b| b.is_ascii_digit()))
//!     }
//!     fn message(&self) -> Option<String> {
//!         Some("Must be a Russian phone number (+7XXXXXXXXXX)".into())
//!     }
//! }
//!
//! let phone = vld::string().trim().rule(RuPhone);
//! assert!(phone.parse(r#""+79991234567""#).is_ok());
//! let err = phone.parse(r#""8999""#).unwrap_err();
//! assert_eq!(err.issues[0].code.key(), "ru_phone");
//!
//! // A closure rule, on any schema.
//! let even = vld::number().int().rule(vld::rules::from_fn("even", |n: &i64| n % 2 == 0));
//! assert!(even.parse("4").is_ok());
//! assert!(even.parse("5").is_err());
//! ```

use serde_json::Value;
use std::sync::Arc;

use crate::error::{IssueCode, ValidationIssue, VldError};

/// A named, reusable check on values of type `T`.
///
/// Implement it on a type (`struct RuPhone;`) and attach it with `.rule(...)`.
/// Rules are shared by schemas that are built once and reused, so they must be
/// `Send + Sync + 'static`.
pub trait Rule<T: ?Sized>: Send + Sync + 'static {
    /// Stable key: the issue code (`IssueCode::Custom { code: key }`), the
    /// `with_messages` key and the translation key.
    fn key(&self) -> &str;

    /// `true` when `value` passes.
    fn check(&self, value: &T) -> bool;

    /// Default message. `None` uses the generic `custom` message, which
    /// translators can replace through the rule's key; `Some` is kept as is.
    fn message(&self) -> Option<String> {
        None
    }

    /// JSON Schema keywords this rule adds (`{"format": "ru-phone"}`,
    /// `{"pattern": "..."}`); merged into the schema it is attached to.
    #[cfg(feature = "openapi")]
    fn json_schema(&self) -> Option<Value> {
        None
    }
}

/// The issue a failing `rule` reports; `msg` (from `rule_msg` /
/// `with_messages`) wins over the rule's own message.
pub(crate) fn failure<T: ?Sized, R: Rule<T> + ?Sized>(
    rule: &R,
    msg: Option<String>,
) -> ValidationIssue {
    let code = IssueCode::Custom {
        code: rule.key().to_string(),
    };
    let msg = msg.or_else(|| rule.message());
    ValidationIssue::with_override(code, msg.as_ref())
}

/// A rule from a closure: `vld::rules::from_fn("even", |n: &i64| n % 2 == 0)`.
pub fn from_fn<T: ?Sized, F>(key: impl Into<String>, check: F) -> FnRule<F>
where
    F: Fn(&T) -> bool + Send + Sync + 'static,
{
    FnRule {
        key: key.into(),
        check,
        message: None,
        #[cfg(feature = "openapi")]
        schema: None,
    }
}

/// Rule built by [`from_fn`].
pub struct FnRule<F> {
    key: String,
    check: F,
    message: Option<String>,
    #[cfg(feature = "openapi")]
    schema: Option<Value>,
}

impl<F> FnRule<F> {
    /// Default message of the rule.
    pub fn message(mut self, msg: impl Into<String>) -> Self {
        self.message = Some(msg.into());
        self
    }

    /// JSON Schema keywords the rule adds (see [`Rule::json_schema`]).
    #[cfg(feature = "openapi")]
    pub fn json_schema(mut self, keywords: Value) -> Self {
        self.schema = Some(keywords);
        self
    }
}

impl<T: ?Sized, F> Rule<T> for FnRule<F>
where
    F: Fn(&T) -> bool + Send + Sync + 'static,
{
    fn key(&self) -> &str {
        &self.key
    }
    fn check(&self, value: &T) -> bool {
        (self.check)(value)
    }
    fn message(&self) -> Option<String> {
        self.message.clone()
    }
    #[cfg(feature = "openapi")]
    fn json_schema(&self) -> Option<Value> {
        self.schema.clone()
    }
}

impl<T: ?Sized, R: Rule<T> + ?Sized> Rule<T> for Arc<R> {
    fn key(&self) -> &str {
        (**self).key()
    }
    fn check(&self, value: &T) -> bool {
        (**self).check(value)
    }
    fn message(&self) -> Option<String> {
        (**self).message()
    }
    #[cfg(feature = "openapi")]
    fn json_schema(&self) -> Option<Value> {
        (**self).json_schema()
    }
}

/// A regular-expression rule for strings (feature `regex`).
///
/// Compiled once when built; matching follows the Rust `regex` syntax (no
/// look-around, no backreferences). Attach it with
/// [`ZString::pattern`](crate::primitives::ZString::pattern).
/// Its issues are `invalid_string` / `StringValidation::Regex`
/// (`with_messages` key `invalid_regex`), and the JSON Schema gets `pattern`.
#[cfg(feature = "regex")]
#[cfg_attr(docsrs, doc(cfg(feature = "regex")))]
#[derive(Clone)]
pub struct Pattern(regex_lite::Regex);

#[cfg(feature = "regex")]
impl Pattern {
    /// Compile `pattern`.
    pub fn new(pattern: &str) -> Result<Self, PatternError> {
        regex_lite::Regex::new(pattern)
            .map(Self)
            .map_err(|e| PatternError(e.to_string()))
    }

    /// The pattern source.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// `true` when `s` matches.
    pub fn is_match(&self, s: &str) -> bool {
        self.0.is_match(s)
    }
}

#[cfg(feature = "regex")]
impl std::fmt::Debug for Pattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Pattern").field(&self.as_str()).finish()
    }
}

/// What [`ZString::pattern`](crate::primitives::ZString::pattern) accepts: a
/// source string (compiled, panics when invalid) or a [`Pattern`].
#[cfg(feature = "regex")]
#[cfg_attr(docsrs, doc(cfg(feature = "regex")))]
pub trait IntoPattern {
    /// The compiled pattern.
    fn into_pattern(self) -> Pattern;
}

#[cfg(feature = "regex")]
impl IntoPattern for Pattern {
    fn into_pattern(self) -> Pattern {
        self
    }
}

#[cfg(feature = "regex")]
impl IntoPattern for &str {
    #[track_caller]
    fn into_pattern(self) -> Pattern {
        Pattern::new(self).unwrap_or_else(|e| panic!("vld: {e} (`{self}`)"))
    }
}

#[cfg(feature = "regex")]
impl IntoPattern for String {
    #[track_caller]
    fn into_pattern(self) -> Pattern {
        self.as_str().into_pattern()
    }
}

/// An invalid regular expression given to [`Pattern::new`].
#[cfg(feature = "regex")]
#[cfg_attr(docsrs, doc(cfg(feature = "regex")))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternError(String);

#[cfg(feature = "regex")]
impl std::fmt::Display for PatternError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid pattern: {}", self.0)
    }
}

#[cfg(feature = "regex")]
impl std::error::Error for PatternError {}

/// Wrapper returned by [`VldSchema::rule`](crate::schema::VldSchema::rule):
/// runs `rule` on the inner schema's output.
pub struct ZRule<S, R> {
    inner: S,
    rule: R,
    msg: Option<String>,
}

impl<S, R> ZRule<S, R> {
    pub(crate) fn new(inner: S, rule: R, msg: Option<String>) -> Self {
        Self { inner, rule, msg }
    }
}

impl<S: crate::schema::VldSchema, R: Rule<S::Output>> ZRule<S, R> {
    fn finish(&self, out: S::Output) -> Result<S::Output, VldError> {
        if self.rule.check(&out) {
            Ok(out)
        } else {
            Err(failure::<S::Output, R>(&self.rule, self.msg.clone()).into())
        }
    }
}

impl<S, R> crate::schema::VldSchema for ZRule<S, R>
where
    S: crate::schema::VldSchema,
    R: Rule<S::Output>,
{
    type Output = S::Output;

    fn parse_value(&self, value: &Value) -> Result<S::Output, VldError> {
        self.finish(self.inner.parse_value(value)?)
    }

    fn parse_missing(&self) -> Result<S::Output, VldError> {
        self.finish(self.inner.parse_missing()?)
    }

    fn output_json(&self, o: &Self::Output) -> Option<Value> {
        self.inner.output_json(o)
    }
}

#[cfg(feature = "openapi")]
impl<S, R> crate::openapi::JsonSchema for ZRule<S, R>
where
    S: crate::openapi::JsonSchema,
    R: Rule<<S as crate::schema::VldSchema>::Output>,
    S: crate::schema::VldSchema,
{
    fn json_schema(&self) -> Value {
        let mut schema = self.inner.json_schema();
        if let Some(extra) = self.rule.json_schema() {
            merge_keywords(&mut schema, extra);
        }
        schema
    }
}

#[cfg(feature = "openapi")]
impl<S: crate::openapi::CollectNestedSchemas, R> crate::openapi::CollectNestedSchemas
    for ZRule<S, R>
{
    fn collect_nested_schemas(&self, out: &mut Vec<crate::openapi::NestedSchemaEntry>) {
        self.inner.collect_nested_schemas(out);
    }
}

/// Merge `extra` keywords into `schema`; a keyword the schema already has
/// goes into `allOf` instead of overwriting it.
#[cfg(feature = "openapi")]
pub(crate) fn merge_keywords(schema: &mut Value, extra: Value) {
    let (Some(base), Value::Object(extra)) = (schema.as_object_mut(), extra) else {
        return;
    };
    let mut clash = serde_json::Map::new();
    for (k, v) in extra {
        if base.contains_key(&k) {
            clash.insert(k, v);
        } else {
            base.insert(k, v);
        }
    }
    if !clash.is_empty() {
        let all_of = base
            .entry("allOf")
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Value::Array(items) = all_of {
            items.push(Value::Object(clash));
        }
    }
}
