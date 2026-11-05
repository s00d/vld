//! [`ZDate`] / [`ZDateTime`] / [`ZTime`], generic over a [`DateBackend`].
//!
//! A backend only provides the concrete types and parsing of an already
//! shape-checked string (see `primitives::iso`); every rule (accepted formats,
//! bounds, past/future, offsets, messages) lives here, so all backends accept
//! exactly the same inputs:
//!
//! - date: `YYYY-MM-DD` (zero-padded, real calendar day);
//! - datetime: `YYYY-MM-DDTHH:MM:SS[.fraction]` (seconds required, 1-9
//!   fraction digits) followed by `Z` or `±HH:MM`; without the offset the input
//!   is *naive* and is accepted only while [`ZDateTime::naive_allowed`] is on;
//! - time of day: `HH:MM:SS[.fraction]` (seconds required, 1-9 fraction
//!   digits, no offset).

use serde_json::Value;
use std::fmt::Display;
use std::marker::PhantomData;

use crate::error::{issue_error, DateKind, IssueCode, SizeOrigin, ValidationIssue, VldError};
use crate::primitives::iso::{is_valid_iso_date, is_valid_time_of_day, split_datetime};
use crate::schema::VldSchema;

mod sealed {
    pub trait Sealed {}
}
pub(super) use sealed::Sealed;

/// A date library behind [`ZDate`] / [`ZDateTime`]: `vld::chrono::Chrono`,
/// `vld::jiff::Jiff` or `vld::time::Time`. Sealed.
pub trait DateBackend: Sealed {
    /// Calendar date type.
    type Date: Clone + PartialOrd + Display;
    /// UTC instant type.
    type DateTime: Clone + PartialOrd;
    /// Time-of-day type.
    type Time: Clone + PartialOrd + Display;

    /// Parse a shape-checked `YYYY-MM-DD` string.
    fn parse_date(s: &str) -> Option<Self::Date>;
    /// Parse a shape-checked `HH:MM:SS[.frac]` string.
    fn parse_time(s: &str) -> Option<Self::Time>;
    /// Interpret a shape-checked `YYYY-MM-DDTHH:MM:SS[.frac]` at `offset_seconds`, as UTC.
    fn naive_to_utc(naive: &str, offset_seconds: i32) -> Option<Self::DateTime>;
    /// Current UTC date.
    fn today() -> Self::Date;
    /// Current instant.
    fn now() -> Self::DateTime;
    /// RFC 3339 form (messages, JSON Schema).
    fn format_datetime(dt: &Self::DateTime) -> String;
    /// `HH:MM:SS[.frac]` form (messages, JSON Schema).
    fn format_time(t: &Self::Time) -> String {
        t.to_string()
    }
}

fn parse_date<B: DateBackend>(s: &str) -> Option<B::Date> {
    is_valid_iso_date(s).then(|| B::parse_date(s)).flatten()
}

fn parse_time<B: DateBackend>(s: &str) -> Option<B::Time> {
    is_valid_time_of_day(s).then(|| B::parse_time(s)).flatten()
}

/// Strict datetime parse; `naive_offset` is applied when the input has no offset
/// (`None` = naive input rejected). Returns the UTC instant and the input offset.
fn parse_datetime<B: DateBackend>(
    s: &str,
    naive_offset: Option<i32>,
) -> Option<(B::DateTime, Option<i32>)> {
    let (naive, offset) = split_datetime(s)?;
    let dt = B::naive_to_utc(&naive, offset.or(naive_offset)?)?;
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

    fn set_min(&mut self, value: T, display: String) {
        self.min = Some(Bound {
            value,
            display,
            msg: None,
        });
    }

    fn set_max(&mut self, value: T, display: String) {
        self.max = Some(Bound {
            value,
            display,
            msg: None,
        });
    }

    fn check(&self, v: &T, now: impl FnOnce() -> T, kind: DateKind, value: &Value) -> VldError {
        let origin = match kind {
            DateKind::Date => SizeOrigin::Date,
            DateKind::DateTime => SizeOrigin::DateTime,
            DateKind::Time => SizeOrigin::Time,
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

    /// `formatMinimum` / `formatMaximum` keywords.
    #[cfg(feature = "openapi")]
    fn json_schema(&self, format: &str) -> Value {
        let mut schema = serde_json::json!({"type": "string", "format": format});
        if let Some(b) = &self.min {
            schema["formatMinimum"] = serde_json::json!(b.display);
        }
        if let Some(b) = &self.max {
            schema["formatMaximum"] = serde_json::json!(b.display);
        }
        schema
    }
}

/// Schema for date validation. Parses strict ISO 8601 dates (`YYYY-MM-DD`)
/// into the backend's date type (`chrono::NaiveDate`, `jiff::civil::Date` or
/// `time::Date`).
///
/// Created via `vld::chrono::date()`, `vld::jiff::date()` or `vld::time::date()`
/// (or `vld::date()` when exactly one backend is enabled).
#[derive(Clone)]
pub struct ZDate<B: DateBackend> {
    bounds: Bounds<B::Date>,
    custom_type_error: Option<String>,
    _backend: PhantomData<fn() -> B>,
}

impl<B: DateBackend> ZDate<B> {
    pub fn new() -> Self {
        Self {
            bounds: Bounds::new(),
            custom_type_error: None,
            _backend: PhantomData,
        }
    }

    /// Set a custom error message for type mismatch (non-string input).
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    fn literal(date: &str) -> B::Date {
        parse_date::<B>(date).unwrap_or_else(|| panic!("Invalid date literal: {}", date))
    }

    /// Minimum date (inclusive). Accepts `"YYYY-MM-DD"`; panics on an invalid literal.
    pub fn min(self, date: &str) -> Self {
        self.min_date(Self::literal(date))
    }

    /// Minimum date (inclusive) from a backend date value.
    pub fn min_date(mut self, date: B::Date) -> Self {
        let display = date.to_string();
        self.bounds.set_min(date, display);
        self
    }

    /// Maximum date (inclusive). Accepts `"YYYY-MM-DD"`; panics on an invalid literal.
    pub fn max(self, date: &str) -> Self {
        self.max_date(Self::literal(date))
    }

    /// Maximum date (inclusive) from a backend date value.
    pub fn max_date(mut self, date: B::Date) -> Self {
        let display = date.to_string();
        self.bounds.set_max(date, display);
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
}

impl<B: DateBackend> Default for ZDate<B> {
    fn default() -> Self {
        Self::new()
    }
}

impl<B: DateBackend> VldSchema for ZDate<B> {
    type Output = B::Date;

    fn output_json(&self, o: &B::Date) -> Option<Value> {
        Some(Value::String(o.to_string()))
    }

    fn parse_value(&self, value: &Value) -> Result<B::Date, VldError> {
        let s = value.as_str().ok_or_else(|| {
            crate::primitives::type_error(
                &self.custom_type_error,
                "date string (YYYY-MM-DD)",
                value,
            )
        })?;
        let date = parse_date::<B>(s).ok_or_else(|| {
            VldError::from_code_with_value(
                IssueCode::InvalidDate {
                    kind: DateKind::Date,
                },
                value,
            )
        })?;
        let errors = self.bounds.check(&date, B::today, DateKind::Date, value);
        if errors.is_empty() {
            Ok(date)
        } else {
            Err(errors)
        }
    }
}

/// `{"type": "string", "format": "date"}`; `min`/`max` become
/// `formatMinimum`/`formatMaximum`.
#[cfg(feature = "openapi")]
impl<B: DateBackend> crate::openapi::JsonSchema for ZDate<B> {
    fn json_schema(&self) -> Value {
        self.bounds.json_schema("date")
    }
}

#[cfg(feature = "openapi")]
impl<B: DateBackend> crate::openapi::CollectNestedSchemas for ZDate<B> {}

/// Offset required by [`ZDateTime::timezone_offset_only`].
#[derive(Clone)]
struct RequiredOffset {
    seconds: i32,
    msg: Option<String>,
}

impl RequiredOffset {
    fn error(&self, value: &Value) -> VldError {
        issue_error(
            IssueCode::InvalidTimezoneOffset {
                expected: format_offset(self.seconds),
            },
            self.msg.as_ref(),
            value,
        )
    }
}

/// Schema for datetime validation. Parses strict RFC 3339 datetimes into the
/// backend's UTC instant (`chrono::DateTime<Utc>`, `jiff::Timestamp` or
/// `time::OffsetDateTime` at UTC).
///
/// Created via `vld::chrono::datetime()`, `vld::jiff::datetime()` or
/// `vld::time::datetime()` (or `vld::datetime()` when exactly one backend is
/// enabled).
///
/// Accepts `YYYY-MM-DDTHH:MM:SS[.fraction]` plus `Z` or `±HH:MM`, e.g.
/// `2024-01-15T10:30:00Z`, `2024-01-15T10:30:00+03:00`,
/// `2024-01-15T10:30:00.123Z`. Naive input (no offset) is accepted by default
/// and interpreted in [`naive_timezone_offset`](Self::naive_timezone_offset)
/// (UTC unless set); see [`naive_allowed`](Self::naive_allowed).
#[derive(Clone)]
pub struct ZDateTime<B: DateBackend> {
    bounds: Bounds<B::DateTime>,
    allow_naive: bool,
    naive_offset: i32,
    required_timezone_offset: Option<RequiredOffset>,
    custom_type_error: Option<String>,
    _backend: PhantomData<fn() -> B>,
}

impl<B: DateBackend> ZDateTime<B> {
    pub fn new() -> Self {
        Self {
            bounds: Bounds::new(),
            allow_naive: true,
            naive_offset: 0,
            required_timezone_offset: None,
            custom_type_error: None,
            _backend: PhantomData,
        }
    }

    /// Set a custom error message for type mismatch (non-string input).
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    fn literal(dt: &str) -> B::DateTime {
        parse_datetime::<B>(dt, None)
            .map(|(dt, _)| dt)
            .unwrap_or_else(|| panic!("Invalid datetime literal: {}", dt))
    }

    /// Minimum datetime (inclusive). Accepts RFC 3339 with offset; panics on an invalid literal.
    pub fn min(self, dt: &str) -> Self {
        self.min_datetime(Self::literal(dt))
    }

    /// Minimum datetime (inclusive) from a backend UTC datetime.
    pub fn min_datetime(mut self, dt: B::DateTime) -> Self {
        let display = B::format_datetime(&dt);
        self.bounds.set_min(dt, display);
        self
    }

    /// Maximum datetime (inclusive). Accepts RFC 3339 with offset; panics on an invalid literal.
    pub fn max(self, dt: &str) -> Self {
        self.max_datetime(Self::literal(dt))
    }

    /// Maximum datetime (inclusive) from a backend UTC datetime.
    pub fn max_datetime(mut self, dt: B::DateTime) -> Self {
        let display = B::format_datetime(&dt);
        self.bounds.set_max(dt, display);
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
}

impl<B: DateBackend> Default for ZDateTime<B> {
    fn default() -> Self {
        Self::new()
    }
}

impl<B: DateBackend> VldSchema for ZDateTime<B> {
    type Output = B::DateTime;

    fn output_json(&self, o: &B::DateTime) -> Option<Value> {
        Some(Value::String(B::format_datetime(o)))
    }

    fn parse_value(&self, value: &Value) -> Result<B::DateTime, VldError> {
        let s = value.as_str().ok_or_else(|| {
            crate::primitives::type_error(&self.custom_type_error, "datetime string", value)
        })?;

        let naive_offset = (self.allow_naive && self.required_timezone_offset.is_none())
            .then_some(self.naive_offset);
        let (dt, offset) = parse_datetime::<B>(s, naive_offset).ok_or_else(|| {
            // A well-formed naive input rejected only because of the offset rule.
            if let (Some(required), Some((_, None))) =
                (&self.required_timezone_offset, split_datetime(s))
            {
                return required.error(value);
            }
            VldError::from_code_with_value(
                IssueCode::InvalidDate {
                    kind: DateKind::DateTime,
                },
                value,
            )
        })?;

        if let Some(required) = &self.required_timezone_offset {
            if offset != Some(required.seconds) {
                return Err(required.error(value));
            }
        }

        let errors = self.bounds.check(&dt, B::now, DateKind::DateTime, value);
        if errors.is_empty() {
            Ok(dt)
        } else {
            Err(errors)
        }
    }
}

/// `{"type": "string", "format": "date-time"}`; `min`/`max` become
/// `formatMinimum`/`formatMaximum`.
#[cfg(feature = "openapi")]
impl<B: DateBackend> crate::openapi::JsonSchema for ZDateTime<B> {
    fn json_schema(&self) -> Value {
        self.bounds.json_schema("date-time")
    }
}

#[cfg(feature = "openapi")]
impl<B: DateBackend> crate::openapi::CollectNestedSchemas for ZDateTime<B> {}

/// Schema for a time of day: `HH:MM:SS[.fraction]` (no offset) into the
/// backend's time type (`chrono::NaiveTime`, `jiff::civil::Time` or
/// `time::Time`).
///
/// Created via `vld::chrono::time()`, `vld::jiff::time()` or `vld::time::time()`
/// (or `vld::time_of_day()` when exactly one backend is enabled). Malformed
/// input fails with `invalid_date` (kind `time`); bounds with `too_small` /
/// `too_big` (origin `time`). JSON Schema: `{"type": "string", "format": "time"}`.
#[derive(Clone)]
pub struct ZTime<B: DateBackend> {
    bounds: Bounds<B::Time>,
    custom_type_error: Option<String>,
    _backend: PhantomData<fn() -> B>,
}

impl<B: DateBackend> ZTime<B> {
    pub fn new() -> Self {
        Self {
            bounds: Bounds::new(),
            custom_type_error: None,
            _backend: PhantomData,
        }
    }

    /// Set a custom error message for type mismatch (non-string input).
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    fn literal(time: &str) -> B::Time {
        parse_time::<B>(time).unwrap_or_else(|| panic!("Invalid time literal: {}", time))
    }

    /// Earliest time (inclusive). Accepts `"HH:MM:SS[.frac]"`; panics on an invalid literal.
    pub fn min(mut self, time: &str) -> Self {
        self.bounds.set_min(Self::literal(time), time.to_string());
        self
    }

    /// Earliest time (inclusive) from a backend time value.
    pub fn min_time(mut self, time: B::Time) -> Self {
        let display = B::format_time(&time);
        self.bounds.set_min(time, display);
        self
    }

    /// Latest time (inclusive). Accepts `"HH:MM:SS[.frac]"`; panics on an invalid literal.
    pub fn max(mut self, time: &str) -> Self {
        self.bounds.set_max(Self::literal(time), time.to_string());
        self
    }

    /// Latest time (inclusive) from a backend time value.
    pub fn max_time(mut self, time: B::Time) -> Self {
        let display = B::format_time(&time);
        self.bounds.set_max(time, display);
        self
    }
}

impl<B: DateBackend> Default for ZTime<B> {
    fn default() -> Self {
        Self::new()
    }
}

impl<B: DateBackend> VldSchema for ZTime<B> {
    type Output = B::Time;

    fn output_json(&self, o: &B::Time) -> Option<Value> {
        Some(Value::String(B::format_time(o)))
    }

    fn parse_value(&self, value: &Value) -> Result<B::Time, VldError> {
        let s = value.as_str().ok_or_else(|| {
            crate::primitives::type_error(&self.custom_type_error, "time string (HH:MM:SS)", value)
        })?;
        let time = parse_time::<B>(s).ok_or_else(|| {
            VldError::from_code_with_value(
                IssueCode::InvalidDate {
                    kind: DateKind::Time,
                },
                value,
            )
        })?;
        // No past/future checks for a time of day, so `now` is never called.
        let errors = self
            .bounds
            .check(&time, || time.clone(), DateKind::Time, value);
        if errors.is_empty() {
            Ok(time)
        } else {
            Err(errors)
        }
    }
}

/// `{"type": "string", "format": "time"}`; `min`/`max` become
/// `formatMinimum`/`formatMaximum`.
#[cfg(feature = "openapi")]
impl<B: DateBackend> crate::openapi::JsonSchema for ZTime<B> {
    fn json_schema(&self) -> Value {
        self.bounds.json_schema("time")
    }
}

#[cfg(feature = "openapi")]
impl<B: DateBackend> crate::openapi::CollectNestedSchemas for ZTime<B> {}
