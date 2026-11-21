//! JSON extraction from a model reply and validation.

use std::fmt;

use serde_json::Value;

use crate::error::VldError;
use crate::schema::{VldParse, VldSchema};

use super::LlmFeedbackOptions;

/// Why a model reply was rejected.
#[derive(Debug, Clone)]
pub enum LlmParseError {
    /// The reply has no JSON object or array.
    NoJson,
    /// The JSON does not pass validation.
    Invalid(VldError),
}

impl LlmParseError {
    /// Feedback for the model: the validation issues
    /// ([`VldError::llm_feedback`]) or a "no JSON" note.
    pub fn llm_feedback(&self) -> String {
        self.llm_feedback_with(&LlmFeedbackOptions::default())
    }

    /// [`llm_feedback`](Self::llm_feedback) with options.
    pub fn llm_feedback_with(&self, options: &LlmFeedbackOptions) -> String {
        match self {
            Self::Invalid(e) => e.llm_feedback_with(options),
            Self::NoJson if options.retry_prompt => {
                "Your previous output contained no JSON. Return the JSON only.".into()
            }
            Self::NoJson => "no JSON object or array found".into(),
        }
    }
}

impl fmt::Display for LlmParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoJson => f.write_str("no JSON object or array found in the reply"),
            Self::Invalid(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for LlmParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NoJson => None,
            Self::Invalid(e) => Some(e),
        }
    }
}

impl From<VldError> for LlmParseError {
    fn from(e: VldError) -> Self {
        Self::Invalid(e)
    }
}

/// The JSON object or array in a model reply: the first one in a fenced
/// block (```` ```json ````), else the first balanced `{…}` / `[…]` that
/// parses, so leading and trailing prose is skipped.
///
/// ```
/// let v = vld::llm::extract_json("Here you go:\n{\"a\": [1, \"}\"]}\nAnything else?").unwrap();
/// assert_eq!(v, serde_json::json!({"a": [1, "}"]}));
/// ```
pub fn extract_json(text: &str) -> Option<Value> {
    let mut rest = text;
    while let Some(start) = rest.find("```") {
        let after = &rest[start + 3..];
        // Skip the info string (`json`) up to the end of the fence line.
        let body_start = after.find('\n').map_or(0, |i| i + 1);
        let Some(len) = after[body_start..].find("```") else {
            break;
        };
        if let Some(v) = first_value(&after[body_start..body_start + len]) {
            return Some(v);
        }
        rest = &after[body_start + len + 3..];
    }
    first_value(text)
}

// ponytail: rescans from each `{` / `[` (quadratic on long prose full of
// brackets); a single pass with a candidate stack if replies get that large.
fn first_value(s: &str) -> Option<Value> {
    s.char_indices()
        .filter(|(_, c)| *c == '{' || *c == '[')
        .find_map(|(i, _)| {
            let end = balanced_end(&s[i..])?;
            serde_json::from_str(&s[i..i + end]).ok()
        })
}

/// Byte length of the bracketed value at the start of `s` (strings skipped).
fn balanced_end(s: &str) -> Option<usize> {
    let (mut depth, mut in_str, mut escaped) = (0usize, false, false);
    for (i, b) in s.bytes().enumerate() {
        if in_str {
            match b {
                _ if escaped => escaped = false,
                b'\\' => escaped = true,
                b'"' => in_str = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// Extract the JSON from a model reply ([`extract_json`]) and parse it as `T`.
///
/// ```
/// vld::schema! {
///     #[derive(Debug)]
///     pub struct Answer { pub score: u8 => vld::u8().max(10) }
/// }
/// let a: Answer = vld::llm::parse_llm("```json\n{\"score\": 7}\n```").unwrap();
/// assert_eq!(a.score, 7);
/// ```
pub fn parse_llm<T: VldParse>(text: &str) -> Result<T, LlmParseError> {
    let value = extract_json(text).ok_or(LlmParseError::NoJson)?;
    Ok(T::vld_parse_value(&value)?)
}

/// [`parse_llm`] with a schema value.
pub fn parse_llm_with<S: VldSchema>(schema: &S, text: &str) -> Result<S::Output, LlmParseError> {
    let value = extract_json(text).ok_or(LlmParseError::NoJson)?;
    Ok(schema.parse_value(&value)?)
}
