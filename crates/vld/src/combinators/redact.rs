use serde_json::Value;

use crate::error::{REDACTED, VldError};
use crate::schema::VldSchema;

/// Keep an inner schema's input out of its errors (passwords, tokens,
/// personal data). Created via [`VldSchema::redact()`].
///
/// Every issue the inner schema reports — nested ones in an object or array
/// included — keeps its `path`, `code` and `message`, but its `received`
/// becomes [`REDACTED`] (`"[redacted]"`), so neither `Display`, `Debug`,
/// a serialized [`VldError`] nor an HTTP error body echoes the value.
/// Built-in messages never quote the input; a custom message that does (a
/// refinement or transform error formatting the value) has each input
/// string of 4+ characters in it replaced by the marker too.
///
/// Redact the outermost schema whose issues may carry the value: a check of
/// an enclosing schema (`array(..).unique()`, a union with no matching
/// variant) reports the value it saw as `received`, outside a redacted
/// element — redact the array / union itself.
///
/// ```
/// use vld::prelude::*;
///
/// let login = vld::object()
///     .field("user", vld::string().min(1))
///     .field("password", vld::string().min(12).redact());
/// let err = login.parse(r#"{"user": "ada", "password": "hunter2"}"#).unwrap_err();
/// assert_eq!(err.issues[0].received, Some(serde_json::json!("[redacted]")));
/// assert!(!err.to_string().contains("hunter2"));
/// ```
pub struct ZRedact<T: VldSchema> {
    inner: T,
}

impl<T: VldSchema> ZRedact<T> {
    /// Create the schema (the same as [`VldSchema::redact()`]).
    pub fn new(inner: T) -> Self {
        Self { inner }
    }

    /// Access the inner schema (for nested OpenAPI component collection).
    pub fn inner_schema(&self) -> &T {
        &self.inner
    }
}

/// Every string of `value`, recursively (object keys excluded).
fn strings<'a>(value: &'a Value, out: &mut Vec<&'a str>) {
    match value {
        Value::String(s) => out.push(s),
        Value::Array(items) => items.iter().for_each(|v| strings(v, out)),
        Value::Object(map) => map.values().for_each(|v| strings(v, out)),
        _ => {}
    }
}

fn redact(mut err: VldError, input: Option<&Value>) -> VldError {
    let mut secrets = vec![];
    if let Some(v) = input {
        strings(v, &mut secrets);
    }
    // Longest first, so a secret containing another is replaced whole.
    secrets.retain(|s| s.chars().count() >= 4);
    secrets.sort_by_key(|s| std::cmp::Reverse(s.len()));
    for issue in &mut err.issues {
        if issue.received.is_some() {
            issue.received = Some(Value::String(REDACTED.to_string()));
        }
        if !issue.has_custom_message() {
            continue;
        }
        for s in &secrets {
            if issue.message.contains(s) {
                issue.message = issue.message.replace(s, REDACTED);
            }
        }
    }
    err
}

impl<T: VldSchema> VldSchema for ZRedact<T> {
    type Output = T::Output;

    fn parse_value(&self, value: &Value) -> Result<T::Output, VldError> {
        self.inner
            .parse_value(value)
            .map_err(|e| redact(e, Some(value)))
    }
    fn parse_missing(&self) -> Result<T::Output, VldError> {
        self.inner.parse_missing().map_err(|e| redact(e, None))
    }
    fn output_json(&self, o: &Self::Output) -> Option<Value> {
        self.inner.output_json(o)
    }
}
