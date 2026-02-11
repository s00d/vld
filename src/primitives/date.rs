use serde_json::Value;

use crate::error::{value_type_name, IssueCode, VldError};
use crate::schema::VldSchema;

/// Schema for date validation. Parses ISO 8601 date strings (`YYYY-MM-DD`)
/// into [`chrono::NaiveDate`].
///
/// Created via [`vld::date()`](crate::date).
///
/// # Example
/// ```ignore
/// use vld::prelude::*;
///
/// let schema = vld::date().min("2020-01-01").max("2030-12-31");
/// let d = schema.parse(r#""2024-06-15""#).unwrap();
/// assert_eq!(d.to_string(), "2024-06-15");
/// ```
#[derive(Clone)]
pub struct ZDate {
    min: Option<(chrono::NaiveDate, String)>,
    max: Option<(chrono::NaiveDate, String)>,
    custom_type_error: Option<String>,
}

impl ZDate {
    pub fn new() -> Self {
        Self {
            min: None,
            max: None,
            custom_type_error: None,
        }
    }

    /// Set a custom error message for type/format mismatch.
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    /// Minimum date (inclusive). Accepts `"YYYY-MM-DD"` string.
    pub fn min(self, date: &str) -> Self {
        let d = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .unwrap_or_else(|_| panic!("Invalid date literal: {}", date));
        self.min_date(d)
    }

    /// Minimum date (inclusive) from a `NaiveDate`.
    pub fn min_date(mut self, date: chrono::NaiveDate) -> Self {
        let msg = format!("Date must be on or after {}", date);
        self.min = Some((date, msg));
        self
    }

    /// Maximum date (inclusive). Accepts `"YYYY-MM-DD"` string.
    pub fn max(self, date: &str) -> Self {
        let d = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .unwrap_or_else(|_| panic!("Invalid date literal: {}", date));
        self.max_date(d)
    }

    /// Maximum date (inclusive) from a `NaiveDate`.
    pub fn max_date(mut self, date: chrono::NaiveDate) -> Self {
        let msg = format!("Date must be on or before {}", date);
        self.max = Some((date, msg));
        self
    }

    /// Generate a JSON Schema representation.
    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "string", "format": "date"})
    }
}

impl Default for ZDate {
    fn default() -> Self {
        Self::new()
    }
}

impl VldSchema for ZDate {
    type Output = chrono::NaiveDate;

    fn parse_value(&self, value: &Value) -> Result<chrono::NaiveDate, VldError> {
        let s = value.as_str().ok_or_else(|| {
            let msg = self.custom_type_error.clone().unwrap_or_else(|| {
                format!(
                    "Expected date string (YYYY-MM-DD), received {}",
                    value_type_name(value)
                )
            });
            VldError::single_with_value(
                IssueCode::InvalidType {
                    expected: "string (date)".to_string(),
                    received: value_type_name(value),
                },
                msg,
                value,
            )
        })?;

        let date = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| {
            VldError::single_with_value(
                IssueCode::Custom {
                    code: "invalid_date".to_string(),
                },
                format!("Invalid date format: expected YYYY-MM-DD, got \"{}\"", s),
                value,
            )
        })?;

        let mut errors = VldError::new();

        if let Some((min_date, msg)) = &self.min {
            if date < *min_date {
                errors.push_with_value(
                    IssueCode::TooSmall {
                        minimum: 0.0,
                        inclusive: true,
                    },
                    msg.clone(),
                    value,
                );
            }
        }
        if let Some((max_date, msg)) = &self.max {
            if date > *max_date {
                errors.push_with_value(
                    IssueCode::TooBig {
                        maximum: 0.0,
                        inclusive: true,
                    },
                    msg.clone(),
                    value,
                );
            }
        }

        if errors.is_empty() {
            Ok(date)
        } else {
            Err(errors)
        }
    }
}

/// Schema for datetime validation. Parses ISO 8601 datetime strings
/// into [`chrono::DateTime<chrono::Utc>`].
///
/// Created via [`vld::datetime()`](crate::datetime).
///
/// Accepts formats like:
/// - `2024-01-15T10:30:00Z`
/// - `2024-01-15T10:30:00+03:00`
/// - `2024-01-15T10:30:00.123Z`
///
/// # Example
/// ```ignore
/// use vld::prelude::*;
///
/// let schema = vld::datetime();
/// let dt = schema.parse(r#""2024-06-15T12:00:00Z""#).unwrap();
/// ```
#[derive(Clone)]
pub struct ZDateTime {
    custom_type_error: Option<String>,
}

impl ZDateTime {
    pub fn new() -> Self {
        Self {
            custom_type_error: None,
        }
    }

    /// Set a custom error message for type/format mismatch.
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    /// Generate a JSON Schema representation.
    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "string", "format": "date-time"})
    }
}

impl Default for ZDateTime {
    fn default() -> Self {
        Self::new()
    }
}

impl VldSchema for ZDateTime {
    type Output = chrono::DateTime<chrono::Utc>;

    fn parse_value(&self, value: &Value) -> Result<chrono::DateTime<chrono::Utc>, VldError> {
        let s = value.as_str().ok_or_else(|| {
            let msg = self.custom_type_error.clone().unwrap_or_else(|| {
                format!(
                    "Expected datetime string, received {}",
                    value_type_name(value)
                )
            });
            VldError::single_with_value(
                IssueCode::InvalidType {
                    expected: "string (datetime)".to_string(),
                    received: value_type_name(value),
                },
                msg,
                value,
            )
        })?;

        use chrono::TimeZone;

        // Try RFC 3339 / ISO 8601 with timezone
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
            return Ok(dt.with_timezone(&chrono::Utc));
        }

        // Try common ISO format without timezone (assume UTC)
        if let Ok(ndt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
            return Ok(chrono::Utc.from_utc_datetime(&ndt));
        }
        if let Ok(ndt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f") {
            return Ok(chrono::Utc.from_utc_datetime(&ndt));
        }

        Err(VldError::single_with_value(
            IssueCode::Custom {
                code: "invalid_datetime".to_string(),
            },
            format!("Invalid datetime format: \"{}\"", s),
            value,
        ))
    }
}
