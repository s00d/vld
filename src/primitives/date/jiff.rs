//! `jiff` date backend: [`date()`], [`datetime()`], [`ZDate`], [`ZDateTime`].
//!
//! ```
//! use vld::prelude::*;
//!
//! let d = vld::jiff::date().parse(r#""2024-06-15""#).unwrap();
//! assert_eq!(d.to_string(), "2024-06-15");
//! ```

use ::jiff::tz::{Offset, TimeZone};

use super::common::Sealed;
use super::DateBackend;

/// The `jiff` backend: `civil::Date` dates, `Timestamp` instants.
#[derive(Debug, Clone, Copy)]
pub struct Jiff;

impl Sealed for Jiff {}

impl DateBackend for Jiff {
    type Date = ::jiff::civil::Date;
    type DateTime = ::jiff::Timestamp;

    fn parse_date(s: &str) -> Option<Self::Date> {
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
