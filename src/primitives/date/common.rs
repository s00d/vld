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

use crate::error::{issue_error, DateKind, IssueCode, SizeOrigin, ValidationIssue, VldError};
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

fn date_error(code: IssueCode, custom: Option<&String>, value: &Value) -> VldError {
    issue_error(code, custom, value)
}

/// A bound: the value, its display form (for messages / JSON Schema) and an
/// optional custom message.
#[derive(Clone)]
struct Bound<T> {
    value: T,
    display: String,
    msg: Option<String>,
}

/// Bounds shared by `ZDate` and `ZDateTime`. Out-of-range values report
/// `TooSmall` / `TooBig` (origin `Date` / `DateTime`, bound as a string);
/// past/future checks report `NotInPast` / `NotInFuture`.
#[derive(Clone)]
struct Bounds<T> {
    min: Option<Bound<T>>,
    max: Option<Bound<T>>,
    /// `Some(custom message)` when the check is on.
    past: Option<Option<String>>,
    future: Option<Option<String>>,
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

    fn check(&self, v: &T, now: impl FnOnce() -> T, kind: DateKind, value: &Value) -> VldError {
        let origin = match kind {
            DateKind::Date => SizeOrigin::Date,
            DateKind::DateTime => SizeOrigin::DateTime,
        };
        let mut errors = VldError::new();
        let mut push = |code, msg: Option<&String>| {
            errors
                .issues
                .push(ValidationIssue::with_override(code, msg).with_received(value))
        };
        if let Some(min) = self.min.as_ref().filter(|b| *v < b.value) {
            push(
                IssueCode::TooSmall {
                    origin,
                    minimum: Value::String(min.display.clone()),
                    inclusive: true,
                },
                min.msg.as_ref(),
            );
        }
        if let Some(max) = self.max.as_ref().filter(|b| *v > b.value) {
            push(
                IssueCode::TooBig {
                    origin,
                    maximum: Value::String(max.display.clone()),
                    inclusive: true,
                },
                max.msg.as_ref(),
            );
        }
        if self.past.is_some() || self.future.is_some() {
            let now = now();
            if let Some(msg) = self.past.as_ref().filter(|_| *v >= now) {
                push(IssueCode::NotInPast { kind }, msg.as_ref());
            }
            if let Some(msg) = self.future.as_ref().filter(|_| *v <= now) {
                push(IssueCode::NotInFuture { kind }, msg.as_ref());
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
        self.bounds.min = Some(Bound {
            display: date.to_string(),
            value: date,
            msg: None,
        });
        self
    }

    /// Maximum date (inclusive). Accepts `"YYYY-MM-DD"` string.
    pub fn max(self, date: &str) -> Self {
        self.max_date(Self::literal(date))
    }

    /// Maximum date (inclusive) from a backend date value.
    pub fn max_date(mut self, date: Date) -> Self {
        self.bounds.max = Some(Bound {
            display: date.to_string(),
            value: date,
            msg: None,
        });
        self
    }

    /// Date must be in the past (before today, UTC).
    pub fn past(mut self) -> Self {
        self.bounds.past = Some(None);
        self
    }

    /// Date must be in the past (before today), with custom message.
    pub fn past_msg(mut self, msg: impl Into<String>) -> Self {
        self.bounds.past = Some(Some(msg.into()));
        self
    }

    /// Date must be in the future (after today, UTC).
    pub fn future(mut self) -> Self {
        self.bounds.future = Some(None);
        self
    }

    /// Date must be in the future (after today), with custom message.
    pub fn future_msg(mut self, msg: impl Into<String>) -> Self {
        self.bounds.future = Some(Some(msg.into()));
        self
    }

    /// Generate a JSON Schema representation.
    ///
    /// `min`/`max` are emitted as `formatMinimum`/`formatMaximum`.
    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        let mut schema = serde_json::json!({"type": "string", "format": "date"});
        if let Some(b) = &self.bounds.min {
            schema["formatMinimum"] = serde_json::json!(b.display);
        }
        if let Some(b) = &self.bounds.max {
            schema["formatMaximum"] = serde_json::json!(b.display);
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
                "date string (YYYY-MM-DD)",
                value,
            )
        })?;
        let date = parse_date(s).ok_or_else(|| {
            date_error(
                IssueCode::InvalidDate {
                    kind: DateKind::Date,
                },
                None,
                value,
            )
        })?;
        let errors = self
            .bounds
            .check(&date, backend::today, DateKind::Date, value);
        if errors.is_empty() {
            Ok(date)
        } else {
            Err(errors)
        }
    }
}

/// Offset required by [`ZDateTime::timezone_offset_only`].
#[derive(Clone)]
struct RequiredOffset {
    seconds: i32,
    msg: Option<String>,
}

impl RequiredOffset {
    fn error(&self, value: &Value) -> VldError {
        date_error(
            IssueCode::InvalidTimezoneOffset {
                expected: format_offset(self.seconds),
            },
            self.msg.as_ref(),
            value,
        )
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
    required_timezone_offset: Option<RequiredOffset>,
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
        self.bounds.min = Some(Bound {
            display: backend::format_datetime(&dt),
            value: dt,
            msg: None,
        });
        self
    }

    /// Maximum datetime (inclusive). Accepts RFC 3339 with offset.
    pub fn max(self, dt: &str) -> Self {
        self.max_datetime(Self::literal(dt))
    }

    /// Maximum datetime (inclusive) from a backend UTC datetime.
    pub fn max_datetime(mut self, dt: DateTime) -> Self {
        self.bounds.max = Some(Bound {
            display: backend::format_datetime(&dt),
            value: dt,
            msg: None,
        });
        self
    }

    /// Datetime must be in the past.
    pub fn past(mut self) -> Self {
        self.bounds.past = Some(None);
        self
    }

    /// Datetime must be in the past, with custom message.
    pub fn past_msg(mut self, msg: impl Into<String>) -> Self {
        self.bounds.past = Some(Some(msg.into()));
        self
    }

    /// Datetime must be in the future.
    pub fn future(mut self) -> Self {
        self.bounds.future = Some(None);
        self
    }

    /// Datetime must be in the future, with custom message.
    pub fn future_msg(mut self, msg: impl Into<String>) -> Self {
        self.bounds.future = Some(Some(msg.into()));
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
    pub fn timezone_offset_only(mut self, offset_seconds: i32) -> Self {
        self.required_timezone_offset = Some(RequiredOffset {
            seconds: check_offset(offset_seconds),
            msg: None,
        });
        self
    }

    /// Same as [`timezone_offset_only`](Self::timezone_offset_only), with custom message.
    pub fn timezone_offset_only_msg(mut self, offset_seconds: i32, msg: impl Into<String>) -> Self {
        self.required_timezone_offset = Some(RequiredOffset {
            seconds: check_offset(offset_seconds),
            msg: Some(msg.into()),
        });
        self
    }

    /// Generate a JSON Schema representation.
    ///
    /// `min`/`max` are emitted as `formatMinimum`/`formatMaximum`.
    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        let mut schema = serde_json::json!({"type": "string", "format": "date-time"});
        if let Some(b) = &self.bounds.min {
            schema["formatMinimum"] = serde_json::json!(b.display);
        }
        if let Some(b) = &self.bounds.max {
            schema["formatMaximum"] = serde_json::json!(b.display);
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
            crate::primitives::type_error(&self.custom_type_error, "datetime string", value)
        })?;

        let naive_offset = (self.allow_naive && self.required_timezone_offset.is_none())
            .then_some(self.naive_offset);
        let (dt, offset) = parse_datetime(s, naive_offset).ok_or_else(|| {
            // A well-formed naive input rejected only because of the offset rule.
            if let (Some(required), Some((_, None))) =
                (&self.required_timezone_offset, split_datetime(s))
            {
                return required.error(value);
            }
            date_error(
                IssueCode::InvalidDate {
                    kind: DateKind::DateTime,
                },
                None,
                value,
            )
        })?;

        if let Some(required) = &self.required_timezone_offset {
            if offset != Some(required.seconds) {
                return Err(required.error(value));
            }
        }

        let errors = self
            .bounds
            .check(&dt, backend::now, DateKind::DateTime, value);
        if errors.is_empty() {
            Ok(dt)
        } else {
            Err(errors)
        }
    }
}
