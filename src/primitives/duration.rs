use serde_json::Value;
use std::time::Duration;

use crate::error::{IssueCode, VldError};
use crate::schema::VldSchema;

fn parse_duration_string(s: &str) -> Option<Duration> {
    if let Some(ms) = s.strip_suffix("ms") {
        return ms.parse::<u64>().ok().map(Duration::from_millis);
    }
    if let Some(sec) = s.strip_suffix('s') {
        return sec.parse::<u64>().ok().map(Duration::from_secs);
    }
    // Minimal ISO-8601: PT{seconds}S
    if let Some(body) = s.strip_prefix("PT").and_then(|v| v.strip_suffix('S')) {
        return body.parse::<u64>().ok().map(Duration::from_secs);
    }
    None
}

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

    /// JSON Schema. `min`/`max` become `minimum`/`maximum` (seconds) on the
    /// integer branch and `x-minSeconds`/`x-maxSeconds` on the string branch.
    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        let mut schema = serde_json::json!({
            "anyOf": [
                { "type": "string", "description": "Duration like `PT15S`, `10s`, `250ms`" },
                { "type": "integer", "minimum": 0, "description": "Duration in seconds" }
            ]
        });
        if let Some(min) = self.min {
            schema["anyOf"][0]["x-minSeconds"] = serde_json::json!(min.as_secs_f64());
            schema["anyOf"][1]["minimum"] = serde_json::json!(min.as_secs_f64().ceil() as u64);
        }
        if let Some(max) = self.max {
            schema["anyOf"][0]["x-maxSeconds"] = serde_json::json!(max.as_secs_f64());
            schema["anyOf"][1]["maximum"] = serde_json::json!(max.as_secs_f64().floor() as u64);
        }
        schema
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
            Value::Number(n) => n.as_u64().map(Duration::from_secs),
            Value::String(s) => parse_duration_string(s),
            _ => None,
        }
        .ok_or_else(|| {
            super::type_error(
                &self.custom_type_error,
                "duration",
                "duration as integer seconds or string (`PT15S`, `10s`, `250ms`)",
                value,
            )
        })?;

        if let Some(min) = self.min {
            if d < min {
                return Err(VldError::single_with_value(
                    IssueCode::TooSmall {
                        minimum: min.as_secs_f64(),
                        inclusive: true,
                    },
                    format!("Duration must be at least {}s", min.as_secs_f64()),
                    value,
                ));
            }
        }
        if let Some(max) = self.max {
            if d > max {
                return Err(VldError::single_with_value(
                    IssueCode::TooBig {
                        maximum: max.as_secs_f64(),
                        inclusive: true,
                    },
                    format!("Duration must be at most {}s", max.as_secs_f64()),
                    value,
                ));
            }
        }
        Ok(d)
    }
}
