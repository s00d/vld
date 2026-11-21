//! [`VldError`] as feedback for a model: one short line per issue.

use serde_json::{Value, json};

use crate::error::{
    IssueCode, PathSegment, REDACTED, SizeOrigin, ValidationIssue, VldError, format_path,
    format_value_short,
};

/// Options of [`VldError::llm_feedback_with`].
#[derive(Debug, Clone)]
pub struct LlmFeedbackOptions {
    /// Issues listed before "and N more" (default 10).
    pub max_issues: usize,
    /// Wrap the lines in a retry instruction ("Your previous output was
    /// invalid: … Return corrected JSON only.").
    pub retry_prompt: bool,
    /// The schema sent to the model (an [`llm_schema`](super::llm_schema)
    /// result): unknown-key issues then list the allowed keys.
    pub schema: Option<Value>,
}

impl Default for LlmFeedbackOptions {
    fn default() -> Self {
        Self {
            max_issues: 10,
            retry_prompt: false,
            schema: None,
        }
    }
}

struct Item {
    path: String,
    expected: String,
    received: Option<String>,
    hint: Option<String>,
}

impl Item {
    fn new(issue: &ValidationIssue, schema: Option<&Value>) -> Self {
        let path = match format_path(&issue.path) {
            p if p.is_empty() => "(root)".to_string(),
            p => p,
        };
        let received = match (&issue.received, &issue.code) {
            (Some(Value::String(s)), _) if s == REDACTED => Some(REDACTED.to_string()),
            (Some(v), _) => Some(format_value_short(v)),
            (None, IssueCode::InvalidType { received, .. }) => Some(received.clone()),
            (None, _) => None,
        };
        let hint = match (&issue.code, schema) {
            (IssueCode::UnrecognizedField, Some(s)) => allowed_keys(s, &issue.path)
                .map(|keys| format!("allowed keys: {}", keys.join(", "))),
            _ => None,
        };
        Self {
            path,
            expected: expected(issue),
            received,
            hint,
        }
    }

    fn line(&self) -> String {
        let mut line = format!("{}: {}", self.path, self.expected);
        if let Some(r) = &self.received {
            line.push_str(", got ");
            line.push_str(r);
        }
        if let Some(h) = &self.hint {
            line.push_str("; ");
            line.push_str(h);
        }
        line
    }
}

fn bound(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// What a size bound measures: `string length`, `number`, `date`.
fn measure(origin: &SizeOrigin) -> String {
    match origin {
        SizeOrigin::String => "string length".into(),
        SizeOrigin::Array => "array length".into(),
        SizeOrigin::Tuple => "tuple length".into(),
        SizeOrigin::Set => "set size".into(),
        SizeOrigin::Record | SizeOrigin::Object => "key count".into(),
        SizeOrigin::Bytes => "byte length".into(),
        SizeOrigin::File => "file size".into(),
        SizeOrigin::Contains => "matching item count".into(),
        SizeOrigin::Scale => "decimal places".into(),
        SizeOrigin::Precision => "significant digits".into(),
        other => other.key().replace('_', " "),
    }
}

/// The rule an issue broke, as the model should read it.
fn expected(issue: &ValidationIssue) -> String {
    use IssueCode::*;
    let rule = match &issue.code {
        InvalidType { expected, .. } => expected.replace('_', " "),
        TooSmall {
            origin,
            minimum,
            inclusive,
        } => {
            let op = if *inclusive { ">=" } else { ">" };
            format!("{} {op} {}", measure(origin), bound(minimum))
        }
        TooBig {
            origin,
            maximum,
            inclusive,
        } => {
            let op = if *inclusive { "<=" } else { "<" };
            format!("{} {op} {}", measure(origin), bound(maximum))
        }
        InvalidLength { origin, expected } => format!("{} = {expected}", measure(origin)),
        InvalidFormat { format } => format!("{} format", format.key().replace('_', " ")),
        InvalidEnumValue { options } => format!("one of {}", options.join(", ")),
        InvalidDiscriminator { options } => {
            let opts: Vec<String> = options.iter().map(bound).collect();
            format!("one of {}", opts.join(", "))
        }
        InvalidLiteral { expected } => format!("exactly {expected}"),
        NotInt => "integer".into(),
        NotFinite => "finite number".into(),
        NotMultipleOf { divisor } => format!("multiple of {}", bound(divisor)),
        NotUnique => "unique items".into(),
        MissingField => return "missing required field".into(),
        UnrecognizedField => return "unknown key, remove it".into(),
        // A rule without a short form: its (possibly custom) message.
        _ => return issue.message.clone(),
    };
    format!("expected {rule}")
}

/// Keys of the object schema that holds the unknown key at `path`.
fn allowed_keys(root: &Value, path: &[PathSegment]) -> Option<Vec<String>> {
    let mut node = resolve(root, root)?;
    for seg in &path[..path.len().saturating_sub(1)] {
        let next = match seg {
            PathSegment::Field(name) => node.get("properties")?.get(name)?,
            PathSegment::Index(_) => node.get("items")?,
        };
        node = resolve(root, next)?;
    }
    Some(
        node.get("properties")?
            .as_object()?
            .keys()
            .cloned()
            .collect(),
    )
}

/// Follow `$ref`s (`#`, `#/$defs/X`) and nullable `anyOf` to the object schema.
fn resolve<'a>(root: &'a Value, mut node: &'a Value) -> Option<&'a Value> {
    // A bound, not a visited set: a `$ref` chain longer than this is a cycle.
    for _ in 0..32 {
        if let Some(r) = node.get("$ref").and_then(Value::as_str) {
            node = if r == "#" {
                root
            } else {
                root.get("$defs")?.get(r.strip_prefix("#/$defs/")?)?
            };
        } else if let Some(branches) = node.get("anyOf").and_then(Value::as_array) {
            node = branches
                .iter()
                .find(|b| b.get("type") != Some(&json!("null")))?;
        } else {
            return Some(node);
        }
    }
    None
}

impl VldError {
    /// The issues as feedback for a model, one line per issue:
    /// `path: expected <rule>, got <value>`, at most 10 lines then
    /// "and N more". See [`llm_feedback_with`](Self::llm_feedback_with).
    ///
    /// ```
    /// use vld::prelude::*;
    /// let err = vld::object()
    ///     .field("name", vld::string().min(2))
    ///     .field("age", vld::number().int())
    ///     .parse(r#"{"name": "A", "age": "x"}"#)
    ///     .unwrap_err();
    /// assert_eq!(
    ///     err.llm_feedback(),
    ///     "name: expected string length >= 2, got \"A\"\nage: expected number, got \"x\"",
    /// );
    /// ```
    pub fn llm_feedback(&self) -> String {
        self.llm_feedback_with(&LlmFeedbackOptions::default())
    }

    /// [`llm_feedback`](Self::llm_feedback) with options: issue limit,
    /// a retry instruction, allowed-key hints from the schema.
    pub fn llm_feedback_with(&self, options: &LlmFeedbackOptions) -> String {
        let items = self.items(options);
        let mut lines: Vec<String> = items.iter().map(Item::line).collect();
        if self.issues.len() > items.len() {
            lines.push(format!("and {} more", self.issues.len() - items.len()));
        }
        let body = lines.join("\n");
        if options.retry_prompt {
            format!("Your previous output was invalid:\n{body}\nReturn corrected JSON only.")
        } else {
            body
        }
    }

    /// The issues as JSON for a tool result:
    /// `[{"path", "expected", "received", "hint"}]` (`received` / `hint` may
    /// be `null`).
    pub fn llm_feedback_json(&self) -> Value {
        self.llm_feedback_json_with(&LlmFeedbackOptions::default())
    }

    /// [`llm_feedback_json`](Self::llm_feedback_json) with options (issue
    /// limit, allowed-key hints).
    pub fn llm_feedback_json_with(&self, options: &LlmFeedbackOptions) -> Value {
        self.items(options)
            .into_iter()
            .map(|i| {
                json!({
                    "path": i.path,
                    "expected": i.expected,
                    "received": i.received,
                    "hint": i.hint,
                })
            })
            .collect()
    }

    fn items(&self, options: &LlmFeedbackOptions) -> Vec<Item> {
        self.issues
            .iter()
            .take(options.max_issues)
            .map(|i| Item::new(i, options.schema.as_ref()))
            .collect()
    }
}
