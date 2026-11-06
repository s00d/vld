//! `jiff` date backend: [`date()`], [`datetime()`], [`time()`], [`ZDate`],
//! [`ZDateTime`], [`ZTime`].
//!
//! ```
//! use vld::prelude::*;
//!
//! let d = vld::jiff::date().parse(r#""2024-06-15""#).unwrap();
//! assert_eq!(d.to_string(), "2024-06-15");
//! ```

use ::jiff::tz::{Offset, TimeZone};

use super::DateBackend;
use super::common::Sealed;

/// The `jiff` backend: `civil::Date` dates, `Timestamp` instants.
#[derive(Debug, Clone, Copy)]
pub struct Jiff;

impl Sealed for Jiff {}

impl DateBackend for Jiff {
    type Date = ::jiff::civil::Date;
    type DateTime = ::jiff::Timestamp;
    type Time = ::jiff::civil::Time;

    fn parse_date(s: &str) -> Option<Self::Date> {
        s.parse().ok()
    }

    fn parse_time(s: &str) -> Option<Self::Time> {
        s.parse().ok()
    }

    fn naive_to_utc(naive: &str, offset_seconds: i32) -> Option<Self::DateTime> {
        let civil: ::jiff::civil::DateTime = naive.parse().ok()?;
        Offset::from_seconds(offset_seconds)
            .ok()?
            .to_timestamp(civil)
            .ok()
    }

    fn today() -> Self::Date {
        ::jiff::Timestamp::now().to_zoned(TimeZone::UTC).date()
    }

    fn now() -> Self::DateTime {
        ::jiff::Timestamp::now()
    }

    fn format_datetime(dt: &Self::DateTime) -> String {
        dt.to_string()
    }
}

/// Date schema producing `jiff::civil::Date`.
pub type ZDate = super::ZDate<Jiff>;

/// Datetime schema producing `jiff::Timestamp`.
pub type ZDateTime = super::ZDateTime<Jiff>;

/// Create a date schema (`YYYY-MM-DD` → `jiff::civil::Date`).
pub fn date() -> ZDate {
    ZDate::new()
}

/// Create a datetime schema (RFC 3339 → `jiff::Timestamp`).
pub fn datetime() -> ZDateTime {
    ZDateTime::new()
}

/// Time-of-day schema producing `jiff::civil::Time`.
pub type ZTime = super::ZTime<Jiff>;

/// Create a time-of-day schema (`HH:MM:SS[.frac]` → `jiff::civil::Time`).
///
/// ```
/// use vld::prelude::*;
///
/// assert!(vld::jiff::time().min("08:00:00").parse(r#""09:30:15.5""#).is_ok());
/// assert!(vld::jiff::time().min("08:00:00").parse(r#""07:59:59""#).is_err());
/// assert!(vld::jiff::time().parse(r#""24:00:00""#).is_err());
/// ```
pub fn time() -> ZTime {
    ZTime::new()
}
