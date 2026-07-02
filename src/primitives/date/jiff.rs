use std::str::FromStr;

use serde_json::Value;

use crate::error::{value_type_name, IssueCode, VldError};
use crate::schema::VldSchema;
use jiff::civil::Date;
use jiff::fmt::temporal::DateTimeParser;
use jiff::tz::{Offset, TimeZone};
use jiff::Timestamp;

fn parse_date_literal(date: &str) -> Date {
    Date::from_str(date).unwrap_or_else(|_| panic!("Invalid date literal: {}", date))
}

fn parse_timestamp_literal(dt: &str) -> Timestamp {
    dt.parse::<Timestamp>()
        .unwrap_or_else(|_| panic!("Invalid datetime literal: {}", dt))
}

fn utc_today() -> Date {
    Timestamp::now().to_zoned(TimeZone::UTC).date()
}

fn fixed_timezone(offset_seconds: i32) -> TimeZone {
    let offset = Offset::from_seconds(offset_seconds).unwrap_or_else(|_| {
        panic!(
            "Invalid timezone offset seconds: {} (expected range -86400..=86400)",
            offset_seconds
        )
    });
    TimeZone::fixed(offset)
}

fn parse_input_offset_seconds(s: &str) -> Option<i32> {
    DateTimeParser::new()
        .parse_pieces(s.as_bytes())
        .ok()
        .and_then(|pieces| pieces.to_numeric_offset())
        .map(|offset| offset.seconds())
}

/// Schema for date validation. Parses ISO 8601 date strings (`YYYY-MM-DD`)
/// into [`jiff::civil::Date`].
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

    /// Minimum date (inclusive) from a `civil::Date`.
    pub fn min_date(mut self, date: Date) -> Self {
        let msg = format!("Date must be on or after {}", date);
        self.min = Some((date, msg));
        self
    }

    /// Maximum date (inclusive). Accepts `"YYYY-MM-DD"` string.
    pub fn max(self, date: &str) -> Self {
        self.max_date(parse_date_literal(date))
    }

    /// Maximum date (inclusive) from a `civil::Date`.
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

        let parser = DateTimeParser::new();
        let date = parser.parse_date(s.as_bytes()).map_err(|_| {
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
/// into [`jiff::Timestamp`].
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
    min: Option<(Timestamp, String)>,
    max: Option<(Timestamp, String)>,
    past: Option<String>,
    future: Option<String>,
    allow_naive: bool,
    naive_offset: Offset,
    required_timezone_offset: Option<(Offset, String)>,
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
            naive_offset: Offset::UTC,
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
        self.min_datetime(parse_timestamp_literal(dt))
    }

    /// Minimum datetime (inclusive) from a `Timestamp`.
    pub fn min_datetime(mut self, dt: Timestamp) -> Self {
        let msg = format!("Datetime must be on or after {}", dt);
        self.min = Some((dt, msg));
        self
    }

    /// Maximum datetime (inclusive). Accepts RFC3339.
    pub fn max(self, dt: &str) -> Self {
        self.max_datetime(parse_timestamp_literal(dt))
    }

    /// Maximum datetime (inclusive) from a `Timestamp`.
    pub fn max_datetime(mut self, dt: Timestamp) -> Self {
        let msg = format!("Datetime must be on or before {}", dt);
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
        self.naive_offset = Offset::from_seconds(offset_seconds).unwrap_or_else(|_| {
            panic!(
                "Invalid timezone offset seconds: {} (expected range -86400..=86400)",
                offset_seconds
            )
        });
        self
    }

    /// Require explicit timezone offset in the input to match `offset_seconds`.
    ///
    /// Applied only to RFC3339 inputs that include timezone.
    pub fn timezone_offset_only(self, offset_seconds: i32) -> Self {
        Offset::from_seconds(offset_seconds).unwrap_or_else(|_| {
            panic!(
                "Invalid timezone offset seconds: {} (expected range -86400..=86400)",
                offset_seconds
            )
        });
        let sign = if offset_seconds >= 0 { '+' } else { '-' };
        let abs = offset_seconds.unsigned_abs();
        let hh = abs / 3600;
        let mm = (abs % 3600) / 60;
        let msg = format!("Timezone offset must be {}{:02}:{:02}", sign, hh, mm);
        self.timezone_offset_only_msg(offset_seconds, msg)
    }

    /// Same as [`timezone_offset_only`](Self::timezone_offset_only), with custom message.
    pub fn timezone_offset_only_msg(mut self, offset_seconds: i32, msg: impl Into<String>) -> Self {
        let offset = Offset::from_seconds(offset_seconds).unwrap_or_else(|_| {
            panic!(
                "Invalid timezone offset seconds: {} (expected range -86400..=86400)",
                offset_seconds
            )
        });
        self.required_timezone_offset = Some((offset, msg.into()));
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
    type Output = Timestamp;

    fn parse_value(&self, value: &Value) -> Result<Timestamp, VldError> {
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

        let parser = DateTimeParser::new();
        let dt = if let Ok(ts) = parser.parse_timestamp(s.as_bytes()) {
            if let Some((required, msg)) = &self.required_timezone_offset {
                let actual = parse_input_offset_seconds(s).ok_or_else(|| {
                    VldError::single_with_value(
                        IssueCode::Custom {
                            code: "invalid_timezone_offset".to_string(),
                        },
                        msg.clone(),
                        value,
                    )
                })?;
                if actual != required.seconds() {
                    return Err(VldError::single_with_value(
                        IssueCode::Custom {
                            code: "invalid_timezone_offset".to_string(),
                        },
                        msg.clone(),
                        value,
                    ));
                }
            }
            ts
        } else if self.allow_naive {
            match parser.parse_datetime(s.as_bytes()) {
                Ok(civil) => civil
                    .to_zoned(fixed_timezone(self.naive_offset.seconds()))
                    .map(|z| z.timestamp())
                    .map_err(|_| {
                        VldError::single_with_value(
                            IssueCode::Custom {
                                code: "invalid_datetime".to_string(),
                            },
                            format!("Invalid datetime format: \"{}\"", s),
                            value,
                        )
                    })?,
                Err(_) => {
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
        let now = Timestamp::now();
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
