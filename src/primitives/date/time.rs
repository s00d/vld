//! [`time`](::time) backend: only types and parsing of pre-validated strings.
//!
//! External crate types use the `::time::` prefix so they are not shadowed by
//! this module.

use ::time::format_description::well_known::Rfc3339;
use ::time::format_description::FormatItem;
use ::time::macros::format_description;
use ::time::{OffsetDateTime, PrimitiveDateTime, UtcOffset};

pub(super) type Date = ::time::Date;
pub(super) type DateTime = OffsetDateTime;

const DATE_FMT: &[FormatItem<'static>] = format_description!("[year]-[month]-[day]");
const NAIVE_FMT: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second][optional [.[subsecond]]]");

pub(super) fn parse_date(s: &str) -> Option<Date> {
    Date::parse(s, DATE_FMT).ok()
}

/// `naive` is `YYYY-MM-DDTHH:MM:SS[.frac]` (already shape-checked).
pub(super) fn naive_to_utc(naive: &str, offset_seconds: i32) -> Option<DateTime> {
    let offset = UtcOffset::from_whole_seconds(offset_seconds).ok()?;
    PrimitiveDateTime::parse(naive, NAIVE_FMT)
        .ok()
        .map(|dt| dt.assume_offset(offset).to_offset(UtcOffset::UTC))
}

pub(super) fn today() -> Date {
    OffsetDateTime::now_utc().date()
}

pub(super) fn now() -> DateTime {
    OffsetDateTime::now_utc()
}

/// RFC 3339; falls back to the `Display` form (never to the current time)
/// for values RFC 3339 cannot represent.
pub(super) fn format_datetime(dt: &DateTime) -> String {
    dt.format(&Rfc3339).unwrap_or_else(|_| dt.to_string())
}
