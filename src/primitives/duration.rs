use serde_json::Value;
use std::time::Duration;

use crate::error::{IssueCode, SizeOrigin, VldError};
use crate::schema::VldSchema;

/// Plain decimal seconds (`5`, `1.5`): digits and at most one `.`, no sign,
/// exponent or `inf`.
fn parse_secs(s: &str) -> Option<Duration> {
    let plain = s.bytes().any(|b| b.is_ascii_digit())
        && s.bytes().all(|b| b.is_ascii_digit() || b == b'.')
        && s.matches('.').count() <= 1;
    if !plain {
        return None;
    }
    match s.parse::<u64>() {
        Ok(n) => Some(Duration::from_secs(n)),
        Err(_) => Duration::try_from_secs_f64(s.parse().ok()?).ok(),
    }
}

fn parse_duration_string(s: &str) -> Option<Duration> {
    if let Some(ms) = s.strip_suffix("ms") {
        return ms.parse::<u64>().ok().map(Duration::from_millis);
    }
    if let Some(sec) = s.strip_suffix('s') {
        return parse_secs(sec);
    }
    // Minimal ISO-8601: PT{seconds}S
    if let Some(body) = s.strip_prefix("PT").and_then(|v| v.strip_suffix('S')) {
        return parse_secs(body);
    }
    None
}

/// Seconds as a JSON number: an integer when whole, else a float.
#[cfg(feature = "openapi")]
fn secs_json(d: Duration) -> Value {
    if d.subsec_nanos() == 0 {
        d.as_secs().into()
    } else {
        d.as_secs_f64().into()
    }
}

/// Schema for `std::time::Duration`. Created via [`vld::duration()`](crate::duration).
///
/// Accepts a non-negative JSON number of seconds (`3`, `1.5`) or a string:
/// `10s` / `1.5s`, `250ms` (integer milliseconds) or `PT15S` / `PT1.5S`.
/// Negative numbers fail with `too_small` (origin `duration`, minimum `0`);
/// anything else with `invalid_type`.
///
/// # Example
/// ```
/// use vld::prelude::*;
/// use std::time::Duration;
///
/// let d = vld::duration().max_secs(60);
/// assert_eq!(d.parse("1.5").unwrap(), Duration::from_millis(1500));
/// assert_eq!(d.parse(r#""PT2.25S""#).unwrap(), Duration::from_millis(2250));
/// assert_eq!(d.parse(r#""250ms""#).unwrap(), Duration::from_millis(250));
/// assert!(d.parse("-1").is_err());
/// ```
#[derive(Clone)]
pub struct ZDuration {
    min: Option<Duration>,
    max: Option<Duration>,
    custom_type_error: Option<String>,
}

impl ZDuration {
    pub fn new() -> Self {
        Self {
            min: None,
            max: None,
            custom_type_error: None,
        }
    }

    /// Set a custom error message for type mismatch.
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    pub fn min(mut self, d: Duration) -> Self {
        self.min = Some(d);
        self
    }

    pub fn max(mut self, d: Duration) -> Self {
        self.max = Some(d);
        self
    }

    pub fn min_secs(self, secs: u64) -> Self {
        self.min(Duration::from_secs(secs))
    }

    pub fn max_secs(self, secs: u64) -> Self {
        self.max(Duration::from_secs(secs))
    }
}

impl Default for ZDuration {
    fn default() -> Self {
        Self::new()
    }
}

impl VldSchema for ZDuration {
    type Output = Duration;

    fn parse_value(&self, value: &Value) -> Result<Duration, VldError> {
        let d = match value {
            Value::Number(n) => match (n.as_u64(), n.as_f64()) {
                (Some(secs), _) => Some(Duration::from_secs(secs)),
                (None, Some(f)) if f < 0.0 => {
                    return Err(VldError::from_code_with_value(
                        IssueCode::TooSmall {
                            origin: SizeOrigin::Duration,
                            minimum: 0.into(),
                            inclusive: true,
                        },
                        value,
                    ))
                }
                (None, Some(f)) => Duration::try_from_secs_f64(f).ok(),
                _ => None,
            },
            Value::String(s) => parse_duration_string(s),
            _ => None,
        }
        .ok_or_else(|| {
            crate::primitives::type_error(
                &self.custom_type_error,
                "duration",
                value,
            )
        })?;

        if let Some(min) = self.min {
            if d < min {
                return Err(VldError::from_code_with_value(
                    IssueCode::TooSmall {
                        origin: SizeOrigin::Duration,
                        minimum: min.as_secs_f64().into(),
                        inclusive: true,
                    },
                    value,
                ));
            }
        }
        if let Some(max) = self.max {
            if d > max {
                return Err(VldError::from_code_with_value(
                    IssueCode::TooBig {
                        origin: SizeOrigin::Duration,
                        maximum: max.as_secs_f64().into(),
                        inclusive: true,
                    },
                    value,
                ));
            }
        }
        Ok(d)
    }
}

/// JSON Schema. `min`/`max` become `minimum`/`maximum` (seconds) on the
/// number branch and `x-minSeconds`/`x-maxSeconds` on the string branch.
#[cfg(feature = "openapi")]
impl crate::openapi::JsonSchema for ZDuration {
    fn json_schema(&self) -> serde_json::Value {
        let mut schema = serde_json::json!({
            "anyOf": [
                { "type": "string", "description": "Duration like `PT15S`, `1.5s`, `250ms`" },
                { "type": "number", "minimum": 0, "description": "Duration in seconds" }
            ]
        });
        if let Some(min) = self.min {
            schema["anyOf"][0]["x-minSeconds"] = serde_json::json!(min.as_secs_f64());
            schema["anyOf"][1]["minimum"] = secs_json(min);
        }
        if let Some(max) = self.max {
            schema["anyOf"][0]["x-maxSeconds"] = serde_json::json!(max.as_secs_f64());
            schema["anyOf"][1]["maximum"] = secs_json(max);
        }
        schema
    }
}

#[cfg(feature = "openapi")]
impl crate::openapi::CollectNestedSchemas for ZDuration {}
