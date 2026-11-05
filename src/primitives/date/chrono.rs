//! `chrono` backend: only types and parsing of pre-validated strings.

use chrono::TimeZone as _;

pub(super) type Date = chrono::NaiveDate;
pub(super) type DateTime = chrono::DateTime<chrono::Utc>;

pub(super) fn parse_date(s: &str) -> Option<Date> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

/// `naive` is `YYYY-MM-DDTHH:MM:SS[.frac]` (already shape-checked).
pub(super) fn naive_to_utc(naive: &str, offset_seconds: i32) -> Option<DateTime> {
    // `%.f` also matches an absent fraction, so one pass covers both forms.
    let ndt = chrono::NaiveDateTime::parse_from_str(naive, "%Y-%m-%dT%H:%M:%S%.f").ok()?;
    chrono::FixedOffset::east_opt(offset_seconds)?
        .from_local_datetime(&ndt)
        .single()
        .map(|dt| dt.with_timezone(&chrono::Utc))
}

pub(super) fn today() -> Date {
    chrono::Utc::now().date_naive()
}

pub(super) fn now() -> DateTime {
    chrono::Utc::now()
}

pub(super) fn format_datetime(dt: &DateTime) -> String {
    dt.to_rfc3339()
}
