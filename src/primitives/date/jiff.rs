//! `jiff` backend: only types and parsing of pre-validated strings.

use jiff::tz::{Offset, TimeZone};

pub(super) type Date = jiff::civil::Date;
pub(super) type DateTime = jiff::Timestamp;

pub(super) fn parse_date(s: &str) -> Option<Date> {
    s.parse().ok()
}

/// `naive` is `YYYY-MM-DDTHH:MM:SS[.frac]` (already shape-checked).
pub(super) fn naive_to_utc(naive: &str, offset_seconds: i32) -> Option<DateTime> {
    let civil: jiff::civil::DateTime = naive.parse().ok()?;
    Offset::from_seconds(offset_seconds)
        .ok()?
        .to_timestamp(civil)
        .ok()
}

pub(super) fn today() -> Date {
    jiff::Timestamp::now().to_zoned(TimeZone::UTC).date()
}

pub(super) fn now() -> DateTime {
    jiff::Timestamp::now()
}

pub(super) fn format_datetime(dt: &DateTime) -> String {
    dt.to_string()
}
