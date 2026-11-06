//! `chrono` date backend: [`date()`], [`datetime()`], [`time()`], [`ZDate`],
//! [`ZDateTime`], [`ZTime`].
//!
//! ```
//! use vld::prelude::*;
//!
//! let d = vld::chrono::date().min("2020-01-01").parse(r#""2024-06-15""#).unwrap();
//! assert_eq!(d, chrono::NaiveDate::from_ymd_opt(2024, 6, 15).unwrap());
//! ```

use ::chrono::TimeZone as _;

use super::DateBackend;
use super::common::Sealed;

/// The `chrono` backend: `NaiveDate` dates, `DateTime<Utc>` instants.
#[derive(Debug, Clone, Copy)]
pub struct Chrono;

impl Sealed for Chrono {}

impl DateBackend for Chrono {
    type Date = ::chrono::NaiveDate;
    type DateTime = ::chrono::DateTime<::chrono::Utc>;
    type Time = ::chrono::NaiveTime;

    fn parse_date(s: &str) -> Option<Self::Date> {
        ::chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
    }

    fn parse_time(s: &str) -> Option<Self::Time> {
        ::chrono::NaiveTime::parse_from_str(s, "%H:%M:%S%.f").ok()
    }

    fn naive_to_utc(naive: &str, offset_seconds: i32) -> Option<Self::DateTime> {
        // `%.f` also matches an absent fraction, so one pass covers both forms.
        let ndt = ::chrono::NaiveDateTime::parse_from_str(naive, "%Y-%m-%dT%H:%M:%S%.f").ok()?;
        ::chrono::FixedOffset::east_opt(offset_seconds)?
            .from_local_datetime(&ndt)
            .single()
            .map(|dt| dt.with_timezone(&::chrono::Utc))
    }

    fn today() -> Self::Date {
        ::chrono::Utc::now().date_naive()
    }

    fn now() -> Self::DateTime {
        ::chrono::Utc::now()
    }

    fn format_datetime(dt: &Self::DateTime) -> String {
        dt.to_rfc3339()
    }
}

/// Date schema producing `chrono::NaiveDate`.
pub type ZDate = super::ZDate<Chrono>;

/// Datetime schema producing `chrono::DateTime<Utc>`.
pub type ZDateTime = super::ZDateTime<Chrono>;

/// Create a date schema (`YYYY-MM-DD` → `chrono::NaiveDate`).
pub fn date() -> ZDate {
    ZDate::new()
}

/// Create a datetime schema (RFC 3339 → `chrono::DateTime<Utc>`).
pub fn datetime() -> ZDateTime {
    ZDateTime::new()
}

/// Time-of-day schema producing `chrono::NaiveTime`.
pub type ZTime = super::ZTime<Chrono>;

/// Create a time-of-day schema (`HH:MM:SS[.frac]` → `chrono::NaiveTime`).
///
/// ```
/// use vld::prelude::*;
///
/// assert!(vld::chrono::time().min("08:00:00").parse(r#""09:30:15.5""#).is_ok());
/// assert!(vld::chrono::time().min("08:00:00").parse(r#""07:59:59""#).is_err());
/// assert!(vld::chrono::time().parse(r#""24:00:00""#).is_err());
/// ```
pub fn time() -> ZTime {
    ZTime::new()
}
