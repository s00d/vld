//! [`time`](::time) backend for [`ZDate`] and [`ZDateTime`].
//!
//! The module is named `time` alongside `chrono` and `jiff`. External crate types use
//! the `::time::` prefix so they are not shadowed by this module.

use ::time::format_description::well_known::Rfc3339;
use ::time::macros::format_description;
use ::time::{Date, OffsetDateTime, PrimitiveDateTime, UtcOffset};

use serde_json::Value;

use crate::error::{value_type_name, IssueCode, VldError};
use crate::schema::VldSchema;

const DATE_FMT: &[::time::format_description::FormatItem<'static>] =
    format_description!("[year]-[month]-[day]");
const NAIVE_FMT: &[::time::format_description::FormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]");
const NAIVE_SUBSEC_FMT: &[::time::format_description::FormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond]");

fn parse_date_literal(date: &str) -> Date {
    Date::parse(date, DATE_FMT).unwrap_or_else(|_| panic!("Invalid date literal: {}", date))
}

fn parse_datetime_literal(dt: &str) -> OffsetDateTime {
    OffsetDateTime::parse(dt, &Rfc3339)
        .map(|parsed| parsed.to_offset(UtcOffset::UTC))
        .unwrap_or_else(|_| panic!("Invalid datetime literal: {}", dt))
}

fn utc_today() -> Date {
    OffsetDateTime::now_utc().date()
}

fn utc_offset(offset_seconds: i32) -> UtcOffset {
    UtcOffset::from_whole_seconds(offset_seconds).unwrap_or_else(|_| {
        panic!(
            "Invalid timezone offset seconds: {} (expected range -86400..=86400)",
            offset_seconds
        )
    })
}

fn format_rfc3339(dt: OffsetDateTime) -> String {
    dt.format(&Rfc3339)
        .unwrap_or_else(|_| OffsetDateTime::now_utc().to_string())
}

fn parse_naive_datetime(s: &str) -> Option<PrimitiveDateTime> {
    PrimitiveDateTime::parse(s, NAIVE_FMT)
        .or_else(|_| PrimitiveDateTime::parse(s, NAIVE_SUBSEC_FMT))
        .ok()
}

/// Schema for date validation. Parses ISO 8601 date strings (`YYYY-MM-DD`)
/// into [`time::Date`].
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
    min: Option<(Date, String)>,
    max: Option<(Date, String)>,
    past: Option<String>,
    future: Option<String>,
    custom_type_error: Option<String>,
}

impl ZDate {
    pub fn new() -> Self {
        Self {
            min: None,
            max: None,
            past: None,
            future: None,
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
        self.min_date(parse_date_literal(date))
    }

    /// Minimum date (inclusive) from a `Date`.
    pub fn min_date(mut self, date: Date) -> Self {
        let msg = format!("Date must be on or after {}", date);
        self.min = Some((date, msg));
        self
    }

    /// Maximum date (inclusive). Accepts `"YYYY-MM-DD"` string.
    pub fn max(self, date: &str) -> Self {
        self.max_date(parse_date_literal(date))
    }

    /// Maximum date (inclusive) from a `Date`.
    pub fn max_date(mut self, date: Date) -> Self {
        let msg = format!("Date must be on or before {}", date);
        self.max = Some((date, msg));
        self
    }

    /// Date must be in the past (before today).
    pub fn past(self) -> Self {
        self.past_msg("Date must be in the past")
    }

    /// Date must be in the past (before today), with custom message.
    pub fn past_msg(mut self, msg: impl Into<String>) -> Self {
        self.past = Some(msg.into());
        self
    }

    /// Date must be in the future (after today).
    pub fn future(self) -> Self {
        self.future_msg("Date must be in the future")
    }

    /// Date must be in the future (after today), with custom message.
    pub fn future_msg(mut self, msg: impl Into<String>) -> Self {
        self.future = Some(msg.into());
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
    type Output = Date;

    fn parse_value(&self, value: &Value) -> Result<Date, VldError> {
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

        let date = Date::parse(s, DATE_FMT).map_err(|_| {
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
        let today = utc_today();
        if let Some(msg) = &self.past {
            if date >= today {
                errors.push_with_value(
                    IssueCode::Custom {
                        code: "not_past_date".to_string(),
                    },
                    msg.clone(),
                    value,
                );
            }
        }
        if let Some(msg) = &self.future {
            if date <= today {
                errors.push_with_value(
                    IssueCode::Custom {
                        code: "not_future_date".to_string(),
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
/// into [`time::OffsetDateTime`] normalized to UTC.
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
    min: Option<(OffsetDateTime, String)>,
    max: Option<(OffsetDateTime, String)>,
    past: Option<String>,
    future: Option<String>,
    allow_naive: bool,
    naive_offset: UtcOffset,
    required_timezone_offset: Option<(UtcOffset, String)>,
    custom_type_error: Option<String>,
}

impl ZDateTime {
    pub fn new() -> Self {
        Self {
            min: None,
            max: None,
            past: None,
            future: None,
            allow_naive: true,
            naive_offset: UtcOffset::UTC,
            required_timezone_offset: None,
            custom_type_error: None,
        }
    }

    /// Set a custom error message for type/format mismatch.
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    /// Minimum datetime (inclusive). Accepts RFC3339.
    pub fn min(self, dt: &str) -> Self {
        self.min_datetime(parse_datetime_literal(dt))
    }

    /// Minimum datetime (inclusive) from a UTC `OffsetDateTime`.
    pub fn min_datetime(mut self, dt: OffsetDateTime) -> Self {
        let msg = format!("Datetime must be on or after {}", format_rfc3339(dt));
        self.min = Some((dt, msg));
        self
    }

    /// Maximum datetime (inclusive). Accepts RFC3339.
    pub fn max(self, dt: &str) -> Self {
        self.max_datetime(parse_datetime_literal(dt))
    }

    /// Maximum datetime (inclusive) from a UTC `OffsetDateTime`.
    pub fn max_datetime(mut self, dt: OffsetDateTime) -> Self {
        let msg = format!("Datetime must be on or before {}", format_rfc3339(dt));
        self.max = Some((dt, msg));
        self
    }

    /// Datetime must be in the past.
    pub fn past(self) -> Self {
        self.past_msg("Datetime must be in the past")
    }

    /// Datetime must be in the past, with custom message.
    pub fn past_msg(mut self, msg: impl Into<String>) -> Self {
        self.past = Some(msg.into());
        self
    }

    /// Datetime must be in the future.
    pub fn future(self) -> Self {
        self.future_msg("Datetime must be in the future")
    }

    /// Datetime must be in the future, with custom message.
    pub fn future_msg(mut self, msg: impl Into<String>) -> Self {
        self.future = Some(msg.into());
        self
    }

    /// Allow or disallow naive datetime input without timezone.
    ///
    /// Default is `true` for backward compatibility.
    pub fn naive_allowed(mut self, allowed: bool) -> Self {
        self.allow_naive = allowed;
        self
    }

    /// Strict mode: require timezone in datetime input (RFC3339).
    pub fn with_timezone_only(self) -> Self {
        self.naive_allowed(false)
    }

    /// Interpret naive datetimes (`YYYY-MM-DDTHH:MM:SS`) in the provided timezone offset.
    ///
    /// The parsed output is still normalized to UTC.
    pub fn naive_timezone_offset(mut self, offset_seconds: i32) -> Self {
        self.naive_offset = utc_offset(offset_seconds);
        self
    }

    /// Require explicit timezone offset in the input to match `offset_seconds`.
    ///
    /// Applied only to RFC3339 inputs that include timezone.
    pub fn timezone_offset_only(self, offset_seconds: i32) -> Self {
        utc_offset(offset_seconds);
        let sign = if offset_seconds >= 0 { '+' } else { '-' };
        let abs = offset_seconds.unsigned_abs();
        let hh = abs / 3600;
        let mm = (abs % 3600) / 60;
        let msg = format!("Timezone offset must be {}{:02}:{:02}", sign, hh, mm);
        self.timezone_offset_only_msg(offset_seconds, msg)
    }

    /// Same as [`timezone_offset_only`](Self::timezone_offset_only), with custom message.
    pub fn timezone_offset_only_msg(mut self, offset_seconds: i32, msg: impl Into<String>) -> Self {
        self.required_timezone_offset = Some((utc_offset(offset_seconds), msg.into()));
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
    type Output = OffsetDateTime;

    fn parse_value(&self, value: &Value) -> Result<OffsetDateTime, VldError> {
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

        let dt = if let Ok(parsed) = OffsetDateTime::parse(s, &Rfc3339) {
            if let Some((required, msg)) = &self.required_timezone_offset {
                if parsed.offset() != *required {
                    return Err(VldError::single_with_value(
                        IssueCode::Custom {
                            code: "invalid_timezone_offset".to_string(),
                        },
                        msg.clone(),
                        value,
                    ));
                }
            }
            parsed.to_offset(UtcOffset::UTC)
        } else if self.allow_naive {
            match parse_naive_datetime(s) {
                Some(naive) => naive
                    .assume_offset(self.naive_offset)
                    .to_offset(UtcOffset::UTC),
                None => {
                    return Err(VldError::single_with_value(
                        IssueCode::Custom {
                            code: "invalid_datetime".to_string(),
                        },
                        format!("Invalid datetime format: \"{}\"", s),
                        value,
                    ));
                }
            }
        } else {
            return Err(VldError::single_with_value(
                IssueCode::Custom {
                    code: "invalid_datetime".to_string(),
                },
                format!("Invalid datetime format: \"{}\"", s),
                value,
            ));
        };

        let mut errors = VldError::new();
        if let Some((min_dt, msg)) = &self.min {
            if dt < *min_dt {
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
        if let Some((max_dt, msg)) = &self.max {
            if dt > *max_dt {
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
        let now = OffsetDateTime::now_utc();
        if let Some(msg) = &self.past {
            if dt >= now {
                errors.push_with_value(
                    IssueCode::Custom {
                        code: "not_past_datetime".to_string(),
                    },
                    msg.clone(),
                    value,
                );
            }
        }
        if let Some(msg) = &self.future {
            if dt <= now {
                errors.push_with_value(
                    IssueCode::Custom {
                        code: "not_future_datetime".to_string(),
                    },
                    msg.clone(),
                    value,
                );
            }
        }

        if errors.is_empty() {
            Ok(dt)
        } else {
            Err(errors)
        }
    }
}
