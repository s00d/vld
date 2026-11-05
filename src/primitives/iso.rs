//! Byte-level ISO 8601 / RFC 3339 shape checks shared by `ZString` formats and
//! the date backends.
//!
//! Everything works on `&[u8]`, so non-ASCII input can never cause a
//! char-boundary panic; `&str` slices are taken only at positions proven to be
//! ASCII.

/// Parse a run of ASCII digits (all bytes must be digits, non-empty).
fn digits(b: &[u8]) -> Option<u32> {
    if b.is_empty() || b.len() > 9 || !b.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(b.iter().fold(0, |acc, d| acc * 10 + u32::from(d - b'0')))
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        _ => 0,
    }
}

/// `YYYY-MM-DD` with a real calendar day (leap years included).
fn date(b: &[u8]) -> bool {
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    match (digits(&b[..4]), digits(&b[5..7]), digits(&b[8..])) {
        (Some(y), Some(m), Some(d)) => (1..=12).contains(&m) && d >= 1 && d <= days_in_month(y, m),
        _ => false,
    }
}

/// `HH:MM[:SS[.frac]]`; with `require_secs` the seconds part is mandatory.
/// Fractions are limited to 1..=9 digits.
fn time(b: &[u8], require_secs: bool) -> bool {
    if b.len() < 5 || b[2] != b':' {
        return false;
    }
    let (Some(h), Some(m)) = (digits(&b[..2]), digits(&b[3..5])) else {
        return false;
    };
    if h > 23 || m > 59 {
        return false;
    }
    let rest = &b[5..];
    if rest.is_empty() {
        return !require_secs;
    }
    if rest.len() < 3 || rest[0] != b':' {
        return false;
    }
    match digits(&rest[1..3]) {
        Some(s) if s <= 59 => {}
        _ => return false,
    }
    let frac = &rest[3..];
    frac.is_empty()
        || (frac[0] == b'.' && (2..=10).contains(&frac.len()) && digits(&frac[1..]).is_some())
}

/// Length of a trailing `Z` / `±HH:MM` offset and its value in seconds.
/// Returns `(0, None)` when there is no offset suffix.
fn offset_suffix(b: &[u8]) -> Option<(usize, Option<i32>)> {
    match b.last() {
        Some(b'Z' | b'z') => return Some((1, Some(0))),
        None => return Some((0, None)),
        _ => {}
    }
    if b.len() < 6 {
        return Some((0, None));
    }
    let o = &b[b.len() - 6..];
    let sign = match o[0] {
        b'+' => 1,
        b'-' => -1,
        _ => return Some((0, None)),
    };
    if o[3] != b':' {
        return None;
    }
    match (digits(&o[1..3]), digits(&o[4..])) {
        (Some(h), Some(m)) if h <= 23 && m <= 59 => {
            Some((6, Some(sign * (h * 3600 + m * 60) as i32)))
        }
        _ => None,
    }
}

pub(crate) fn is_valid_iso_date(s: &str) -> bool {
    date(s.as_bytes())
}

pub(crate) fn is_valid_iso_time(s: &str) -> bool {
    time(s.as_bytes(), false)
}

/// `YYYY-MM-DDTHH:MM[:SS[.frac]][Z|±HH:MM]`.
pub(crate) fn is_valid_iso_datetime(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() < 16 || b[10] != b'T' || !date(&b[..10]) {
        return false;
    }
    match offset_suffix(&b[11..]) {
        Some((n, _)) => time(&b[11..b.len() - n], false),
        None => false,
    }
}

/// Strict RFC 3339 split used by the date backends:
/// `YYYY-MM-DDTHH:MM:SS[.frac][Z|±HH:MM]` (`T`/`Z` may be lowercase).
///
/// Returns the naive part normalized to an uppercase `T`
/// (`YYYY-MM-DDTHH:MM:SS[.frac]`) and the offset in seconds, if present.
#[cfg(any(feature = "chrono", feature = "jiff", feature = "time"))]
pub(crate) fn split_datetime(s: &str) -> Option<(String, Option<i32>)> {
    let b = s.as_bytes();
    if b.len() < 19 || !matches!(b[10], b'T' | b't') || !date(&b[..10]) {
        return None;
    }
    let (n, offset) = offset_suffix(&b[11..])?;
    let end = b.len() - n;
    if !time(&b[11..end], true) {
        return None;
    }
    // All bytes in 0..end are ASCII (checked above), so slicing is safe.
    Some((format!("{}T{}", &s[..10], &s[11..end]), offset))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_ascii_never_panics() {
        for s in [
            "123é56789",
            "1é:00",
            "2024-01-01T",
            "2024-01-01Té",
            "2024-01-01T1é:00:00Z",
            "é",
        ] {
            assert!(!is_valid_iso_date(s));
            assert!(!is_valid_iso_time(s));
            assert!(!is_valid_iso_datetime(s));
        }
    }

    #[test]
    fn calendar_and_offsets() {
        assert!(is_valid_iso_date("2024-02-29"));
        assert!(!is_valid_iso_date("2023-02-29"));
        assert!(!is_valid_iso_date("2024-02-31"));
        assert!(!is_valid_iso_date("2024-04-31"));
        assert!(is_valid_iso_datetime("2024-01-01T10:00+03:00"));
        assert!(is_valid_iso_datetime("2024-01-01T10:00:00.5Z"));
        assert!(!is_valid_iso_datetime("2024-01-01T10:00+garbage"));
        assert!(!is_valid_iso_datetime("2024-01-01T10:00Zjunk"));
        assert!(!is_valid_iso_datetime("2024-01-01T10:00+25:00"));
    }
}
