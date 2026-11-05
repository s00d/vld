//! `ZDate` / `ZDateTime`, shared by all date backends.
//!
//! Backend priority when multiple date features are enabled (e.g. `--all-features`):
//! `chrono` > `jiff` > `time`. A backend only provides the concrete types and
//! parsing of an already shape-checked string (see [`super::iso`]); every rule
//! (accepted formats, bounds, past/future, offsets, messages) lives here, so
//! all backends accept exactly the same inputs:
//!
//! - date: `YYYY-MM-DD` (zero-padded, real calendar day);
//! - datetime: `YYYY-MM-DDTHH:MM:SS[.fraction]` (seconds required, 1-9
//!   fraction digits) followed by `Z` or `±HH:MM`; without the offset the input
//!   is *naive* and is accepted only while [`ZDateTime::naive_allowed`] is on.

use serde_json::Value;

use crate::error::{IssueCode, VldError};
use crate::schema::VldSchema;

use crate::primitives::iso::{is_valid_iso_date, split_datetime};

#[cfg(feature = "chrono")]
#[path = "chrono.rs"]
mod backend;
#[cfg(all(feature = "jiff", not(feature = "chrono")))]
#[path = "jiff.rs"]
mod backend;
#[cfg(all(feature = "time", not(any(feature = "chrono", feature = "jiff"))))]
#[path = "time.rs"]
mod backend;

use backend::{Date, DateTime};

fn parse_date(s: &str) -> Option<Date> {
    is_valid_iso_date(s)
        .then(|| backend::parse_date(s))
        .flatten()
}

/// Strict datetime parse; `naive_offset` is applied when the input has no offset
/// (`None` = naive input rejected). Returns the UTC instant and the input offset.
fn parse_datetime(s: &str, naive_offset: Option<i32>) -> Option<(DateTime, Option<i32>)> {
    let (naive, offset) = split_datetime(s)?;
    let dt = backend::naive_to_utc(&naive, offset.or(naive_offset)?)?;
    Some((dt, offset))
}

fn check_offset(offset_seconds: i32) -> i32 {
    if offset_seconds.unsigned_abs() >= 86_400 {
        panic!(
            "Invalid timezone offset seconds: {} (expected range -86399..=86399)",
            offset_seconds
        );
    }
    offset_seconds
}

fn format_offset(offset_seconds: i32) -> String {
    let sign = if offset_seconds >= 0 { '+' } else { '-' };
    let abs = offset_seconds.unsigned_abs();
    format!("{}{:02}:{:02}", sign, abs / 3600, (abs % 3600) / 60)
}

fn custom(code: &str, msg: &str, value: &Value) -> VldError {
    VldError::single_with_value(
        IssueCode::Custom {
            code: code.to_string(),
        },
        msg,
        value,
    )
}

/// Bounds shared by `ZDate` and `ZDateTime`.
///
/// Out-of-range values report `Custom { code: "date_too_small" | "date_too_big" }`
/// (instead of `TooSmall`/`TooBig`, whose numeric `minimum`/`maximum` cannot
/// hold a date), so translated messages never show a bogus `0` bound.
#[derive(Clone)]
struct Bounds<T> {
    min: Option<(T, String)>,
    max: Option<(T, String)>,
    past: Option<String>,
    future: Option<String>,
}

impl<T: PartialOrd> Bounds<T> {
    fn new() -> Self {
        Self {
            min: None,
            max: None,
            past: None,
            future: None,
        }
    }

    fn check(&self, v: &T, now: impl FnOnce() -> T, kind: &str, value: &Value) -> VldError {
        let mut errors = VldError::new();
        let mut push = |code: String, msg: &String| {
            errors.push_with_value(IssueCode::Custom { code }, msg.clone(), value)
        };
        if let Some((min, msg)) = &self.min {
            if v < min {
                push("date_too_small".into(), msg);
            }
        }
        if let Some((max, msg)) = &self.max {
            if v > max {
                push("date_too_big".into(), msg);
            }
        }
        if self.past.is_some() || self.future.is_some() {
            let now = now();
            if let Some(msg) = self.past.as_ref().filter(|_| *v >= now) {
                push(format!("not_past_{}", kind), msg);
            }
            if let Some(msg) = self.future.as_ref().filter(|_| *v <= now) {
                push(format!("not_future_{}", kind), msg);
            }
        }
        errors
    }
}

/// Schema for date validation. Parses strict ISO 8601 dates (`YYYY-MM-DD`)
/// into the active backend's date type (`chrono::NaiveDate`,
/// `jiff::civil::Date` or `time::Date`).
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
    bounds: Bounds<Date>,
    custom_type_error: Option<String>,
}

impl ZDate {
    pub fn new() -> Self {
        Self {
            bounds: Bounds::new(),
            custom_type_error: None,
        }
    }

    /// Set a custom error message for type/format mismatch.
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    fn literal(date: &str) -> Date {
        parse_date(date).unwrap_or_else(|| panic!("Invalid date literal: {}", date))
    }

    /// Minimum date (inclusive). Accepts `"YYYY-MM-DD"` string.
    pub fn min(self, date: &str) -> Self {
        self.min_date(Self::literal(date))
    }

    /// Minimum date (inclusive) from a backend date value.
    pub fn min_date(mut self, date: Date) -> Self {
        let msg = format!("Date must be on or after {}", date);
        self.bounds.min = Some((date, msg));
        self
    }

    /// Maximum date (inclusive). Accepts `"YYYY-MM-DD"` string.
    pub fn max(self, date: &str) -> Self {
        self.max_date(Self::literal(date))
    }

    /// Maximum date (inclusive) from a backend date value.
    pub fn max_date(mut self, date: Date) -> Self {
        let msg = format!("Date must be on or before {}", date);
        self.bounds.max = Some((date, msg));
        self
    }

    /// Date must be in the past (before today, UTC).
    pub fn past(self) -> Self {
        self.past_msg("Date must be in the past")
    }

    /// Date must be in the past (before today), with custom message.
    pub fn past_msg(mut self, msg: impl Into<String>) -> Self {
        self.bounds.past = Some(msg.into());
        self
    }

    /// Date must be in the future (after today, UTC).
    pub fn future(self) -> Self {
        self.future_msg("Date must be in the future")
    }

    /// Date must be in the future (after today), with custom message.
    pub fn future_msg(mut self, msg: impl Into<String>) -> Self {
        self.bounds.future = Some(msg.into());
        self
    }

    /// Generate a JSON Schema representation.
    ///
    /// `min`/`max` are emitted as `formatMinimum`/`formatMaximum`.
    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        let mut schema = serde_json::json!({"type": "string", "format": "date"});
        if let Some((d, _)) = &self.bounds.min {
            schema["formatMinimum"] = serde_json::json!(d.to_string());
        }
        if let Some((d, _)) = &self.bounds.max {
            schema["formatMaximum"] = serde_json::json!(d.to_string());
        }
        schema
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
            crate::primitives::type_error(
                &self.custom_type_error,
                "string (date)",
                "date string (YYYY-MM-DD)",
                value,
            )
        })?;
        let date = parse_date(s).ok_or_else(|| {
            custom(
                "invalid_date",
                &format!("Invalid date format: expected YYYY-MM-DD, got \"{}\"", s),
                value,
            )
        })?;
        let errors = self.bounds.check(&date, backend::today, "date", value);
        if errors.is_empty() {
            Ok(date)
        } else {
            Err(errors)
        }
    }
}

/// Schema for datetime validation. Parses strict RFC 3339 datetimes into the
/// active backend's UTC instant (`chrono::DateTime<Utc>`, `jiff::Timestamp`
/// or `time::OffsetDateTime` at UTC).
///
/// Created via [`vld::datetime()`](crate::datetime).
///
/// Accepts `YYYY-MM-DDTHH:MM:SS[.fraction]` plus `Z` or `±HH:MM`, e.g.
/// `2024-01-15T10:30:00Z`, `2024-01-15T10:30:00+03:00`,
/// `2024-01-15T10:30:00.123Z`. Naive input (no offset) is accepted by default
/// and interpreted in [`naive_timezone_offset`](Self::naive_timezone_offset)
/// (UTC unless set); see [`naive_allowed`](Self::naive_allowed).
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
    bounds: Bounds<DateTime>,
    allow_naive: bool,
    naive_offset: i32,
    required_timezone_offset: Option<(i32, String)>,
    custom_type_error: Option<String>,
}

impl ZDateTime {
    pub fn new() -> Self {
        Self {
            bounds: Bounds::new(),
            allow_naive: true,
            naive_offset: 0,
            required_timezone_offset: None,
            custom_type_error: None,
        }
    }

    /// Set a custom error message for type/format mismatch.
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    fn literal(dt: &str) -> DateTime {
        parse_datetime(dt, None)
            .map(|(dt, _)| dt)
            .unwrap_or_else(|| panic!("Invalid datetime literal: {}", dt))
    }

    /// Minimum datetime (inclusive). Accepts RFC 3339 with offset.
    pub fn min(self, dt: &str) -> Self {
        self.min_datetime(Self::literal(dt))
    }

    /// Minimum datetime (inclusive) from a backend UTC datetime.
    pub fn min_datetime(mut self, dt: DateTime) -> Self {
        let msg = format!(
            "Datetime must be on or after {}",
            backend::format_datetime(&dt)
        );
        self.bounds.min = Some((dt, msg));
        self
    }

    /// Maximum datetime (inclusive). Accepts RFC 3339 with offset.
    pub fn max(self, dt: &str) -> Self {
        self.max_datetime(Self::literal(dt))
    }

    /// Maximum datetime (inclusive) from a backend UTC datetime.
    pub fn max_datetime(mut self, dt: DateTime) -> Self {
        let msg = format!(
            "Datetime must be on or before {}",
            backend::format_datetime(&dt)
        );
        self.bounds.max = Some((dt, msg));
        self
    }

    /// Datetime must be in the past.
    pub fn past(self) -> Self {
        self.past_msg("Datetime must be in the past")
    }

    /// Datetime must be in the past, with custom message.
    pub fn past_msg(mut self, msg: impl Into<String>) -> Self {
        self.bounds.past = Some(msg.into());
        self
    }

    /// Datetime must be in the future.
    pub fn future(self) -> Self {
        self.future_msg("Datetime must be in the future")
    }

    /// Datetime must be in the future, with custom message.
    pub fn future_msg(mut self, msg: impl Into<String>) -> Self {
        self.bounds.future = Some(msg.into());
        self
    }

    /// Allow or disallow naive datetime input without timezone.
    ///
    /// Default is `true` for backward compatibility. Ignored (naive input is
    /// always rejected) once [`timezone_offset_only`](Self::timezone_offset_only) is set.
    pub fn naive_allowed(mut self, allowed: bool) -> Self {
        self.allow_naive = allowed;
        self
    }

    /// Strict mode: require timezone in datetime input (RFC 3339).
    pub fn with_timezone_only(self) -> Self {
        self.naive_allowed(false)
    }

    /// Interpret naive datetimes (`YYYY-MM-DDTHH:MM:SS`) in the provided timezone offset.
    ///
    /// The parsed output is still normalized to UTC.
    ///
    /// # Panics
    /// If `|offset_seconds| >= 86400`.
    pub fn naive_timezone_offset(mut self, offset_seconds: i32) -> Self {
        self.naive_offset = check_offset(offset_seconds);
        self
    }

    /// Require the input to carry an explicit offset equal to `offset_seconds`.
    ///
    /// Naive input (no offset) is rejected as well — the offset cannot be
    /// verified — regardless of [`naive_allowed`](Self::naive_allowed).
    ///
    /// # Panics
    /// If `|offset_seconds| >= 86400`.
    pub fn timezone_offset_only(self, offset_seconds: i32) -> Self {
        let msg = format!(
            "Timezone offset must be {}",
            format_offset(check_offset(offset_seconds))
        );
        self.timezone_offset_only_msg(offset_seconds, msg)
    }

    /// Same as [`timezone_offset_only`](Self::timezone_offset_only), with custom message.
    pub fn timezone_offset_only_msg(mut self, offset_seconds: i32, msg: impl Into<String>) -> Self {
        self.required_timezone_offset = Some((check_offset(offset_seconds), msg.into()));
        self
    }

    /// Generate a JSON Schema representation.
    ///
    /// `min`/`max` are emitted as `formatMinimum`/`formatMaximum`.
    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        let mut schema = serde_json::json!({"type": "string", "format": "date-time"});
        if let Some((d, _)) = &self.bounds.min {
            schema["formatMinimum"] = serde_json::json!(backend::format_datetime(d));
        }
        if let Some((d, _)) = &self.bounds.max {
            schema["formatMaximum"] = serde_json::json!(backend::format_datetime(d));
        }
        schema
    }
}

impl Default for ZDateTime {
    fn default() -> Self {
        Self::new()
    }
}

impl VldSchema for ZDateTime {
    type Output = DateTime;

    fn parse_value(&self, value: &Value) -> Result<DateTime, VldError> {
        let s = value.as_str().ok_or_else(|| {
            crate::primitives::type_error(
                &self.custom_type_error,
                "string (datetime)",
                "datetime string",
                value,
            )
        })?;

        let naive_offset = (self.allow_naive && self.required_timezone_offset.is_none())
            .then_some(self.naive_offset);
        let (dt, offset) = parse_datetime(s, naive_offset).ok_or_else(|| {
            // A well-formed naive input rejected only because of the offset rule.
            if let (Some((_, msg)), Some((_, None))) =
                (&self.required_timezone_offset, split_datetime(s))
            {
                return custom("invalid_timezone_offset", msg, value);
            }
            custom(
                "invalid_datetime",
                &format!("Invalid datetime format: \"{}\"", s),
                value,
            )
        })?;

        if let Some((required, msg)) = &self.required_timezone_offset {
            if offset != Some(*required) {
                return Err(custom("invalid_timezone_offset", msg, value));
            }
        }

        let errors = self.bounds.check(&dt, backend::now, "datetime", value);
        if errors.is_empty() {
            Ok(dt)
        } else {
            Err(errors)
        }
    }
}
