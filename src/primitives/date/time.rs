//! `time` date backend: [`date()`], [`datetime()`], [`ZDate`], [`ZDateTime`].
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
const NAIVE_FMT: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second][optional [.[subsecond]]]");

/// The `time` backend: `Date` dates, `OffsetDateTime` instants (at UTC).
#[derive(Debug, Clone, Copy)]
pub struct Time;

impl Sealed for Time {}

impl DateBackend for Time {
    type Date = ::time::Date;
    type DateTime = OffsetDateTime;

    fn parse_date(s: &str) -> Option<Self::Date> {
        ::time::Date::parse(s, DATE_FMT).ok()
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
