//! `time` date backend: [`date()`], [`datetime()`], [`time()`], [`ZDate`],
//! [`ZDateTime`], [`ZTime`].
//!
//! ```
//! use vld::prelude::*;
//!
//! let d = vld::time::date().parse(r#""2024-06-15""#).unwrap();
//! assert_eq!(d.to_string(), "2024-06-15");
//! ```

use ::time::format_description::well_known::Rfc3339;
use ::time::format_description::FormatItem;
use ::time::macros::format_description;
use ::time::{OffsetDateTime, PrimitiveDateTime, UtcOffset};

use super::common::Sealed;
use super::DateBackend;

const DATE_FMT: &[FormatItem<'static>] = format_description!("[year]-[month]-[day]");
const TIME_FMT: &[FormatItem<'static>] =
    format_description!("[hour]:[minute]:[second][optional [.[subsecond]]]");
const TIME_OUT_FMT: &[FormatItem<'static>] =
    format_description!("[hour]:[minute]:[second].[subsecond]");
const NAIVE_FMT: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second][optional [.[subsecond]]]");

/// The `time` backend: `Date` dates, `OffsetDateTime` instants (at UTC).
#[derive(Debug, Clone, Copy)]
pub struct Time;

impl Sealed for Time {}

impl DateBackend for Time {
    type Date = ::time::Date;
    type DateTime = OffsetDateTime;
    type Time = ::time::Time;

    fn parse_date(s: &str) -> Option<Self::Date> {
        ::time::Date::parse(s, DATE_FMT).ok()
    }

    fn parse_time(s: &str) -> Option<Self::Time> {
        ::time::Time::parse(s, TIME_FMT).ok()
    }

    fn naive_to_utc(naive: &str, offset_seconds: i32) -> Option<Self::DateTime> {
        let offset = UtcOffset::from_whole_seconds(offset_seconds).ok()?;
        PrimitiveDateTime::parse(naive, NAIVE_FMT)
            .ok()
            .map(|dt| dt.assume_offset(offset).to_offset(UtcOffset::UTC))
    }

    fn today() -> Self::Date {
        OffsetDateTime::now_utc().date()
    }

    fn now() -> Self::DateTime {
        OffsetDateTime::now_utc()
    }

    /// `HH:MM:SS.frac` (`Display` does not zero-pad the hour).
    fn format_time(t: &Self::Time) -> String {
        t.format(TIME_OUT_FMT).unwrap_or_else(|_| t.to_string())
    }

    /// RFC 3339; falls back to the `Display` form (never to the current time)
    /// for values RFC 3339 cannot represent.
    fn format_datetime(dt: &Self::DateTime) -> String {
        dt.format(&Rfc3339).unwrap_or_else(|_| dt.to_string())
    }
}

/// Date schema producing `time::Date`.
pub type ZDate = super::ZDate<Time>;

/// Datetime schema producing `time::OffsetDateTime`.
pub type ZDateTime = super::ZDateTime<Time>;

/// Create a date schema (`YYYY-MM-DD` → `time::Date`).
pub fn date() -> ZDate {
    ZDate::new()
}

/// Create a datetime schema (RFC 3339 → `time::OffsetDateTime`).
pub fn datetime() -> ZDateTime {
    ZDateTime::new()
}

/// Time-of-day schema producing `time::Time`.
pub type ZTime = super::ZTime<Time>;

/// Create a time-of-day schema (`HH:MM:SS[.frac]` → `time::Time`).
///
/// ```
/// use vld::prelude::*;
///
/// assert!(vld::time::time().min("08:00:00").parse(r#""09:30:15.5""#).is_ok());
/// assert!(vld::time::time().min("08:00:00").parse(r#""07:59:59""#).is_err());
/// assert!(vld::time::time().parse(r#""24:00:00""#).is_err());
/// ```
pub fn time() -> ZTime {
    ZTime::new()
}
