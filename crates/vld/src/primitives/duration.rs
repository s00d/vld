use serde_json::Value;
use std::time::Duration;

use crate::error::{FormatKind, IssueCode, SizeOrigin, VldError};
use crate::schema::VldSchema;

/// Plain decimal seconds (`5`, `1.5`): digits and at most one `.`, no sign,
/// exponent or `inf`.
fn parse_secs(s: &str) -> Option<Duration> {
    let plain = s.bytes().any(|b| b.is_ascii_digit())
        && s.bytes().all(|b| b.is_ascii_digit() || b == b'.')
        && s.matches('.').count() <= 1;
    if !plain {
        return None;
    }
    match s.parse::<u64>() {
        Ok(n) => Some(Duration::from_secs(n)),
        Err(_) => Duration::try_from_secs_f64(s.parse().ok()?).ok(),
    }
}

/// Why a duration string was rejected.
enum Rejected {
    Invalid,
    /// A well-formed ISO 8601 duration with years or months.
    CalendarUnit,
}

fn parse_duration_string(s: &str) -> Result<Duration, Rejected> {
    if let Some(body) = s.strip_prefix('P') {
        return parse_iso(body);
    }
    if let Some(ms) = s.strip_suffix("ms") {
        // Digits only: `u64::from_str` would accept a leading `+`.
        if ms.is_empty() || !ms.bytes().all(|b| b.is_ascii_digit()) {
            return Err(Rejected::Invalid);
        }
        return ms
            .parse::<u64>()
            .map(Duration::from_millis)
            .map_err(|_| Rejected::Invalid);
    }
    s.strip_suffix('s')
        .and_then(parse_secs)
        .ok_or(Rejected::Invalid)
}

const NANOS_PER_SEC: u128 = 1_000_000_000;

/// ISO 8601 / RFC 3339 duration after the `P`: `nW` alone, or
/// `[nD][T[nH][nM][nS]]` (at least one part, units in that order, `T` only
/// before a time part). Only the last number may have a fraction (`.` or
/// `,`, at most 9 digits). A day is 86 400 s. Years and months (`nY`, `nM`
/// before `T`) are not a fixed length: [`Rejected::CalendarUnit`].
fn parse_iso(body: &str) -> Result<Duration, Rejected> {
    use Rejected::Invalid;
    let (date, time) = match body.split_once('T') {
        Some((_, "")) => return Err(Invalid),
        Some((d, t)) => (d, Some(t)),
        None => (body, None),
    };
    // (unit letter, seconds, part of the time section)
    const UNITS: &[(u8, u64, bool)] = &[
        (b'Y', 0, false),
        (b'M', 0, false),
        (b'W', 604_800, false),
        (b'D', 86_400, false),
        (b'H', 3_600, true),
        (b'M', 60, true),
        (b'S', 1, true),
    ];
    let mut parts = Vec::new(); // (unit index, integer digits, fraction digits)
    for (text, in_time) in [(date, false), (time.unwrap_or(""), true)] {
        let mut rest = text;
        while !rest.is_empty() {
            let end = rest
                .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ','))
                .ok_or(Invalid)?;
            let (num, unit) = (&rest[..end], rest.as_bytes()[end]);
            let (int, frac) = match num.split_once(['.', ',']) {
                Some((i, f)) => (i, Some(f)),
                None => (num, None),
            };
            let digits = |d: &str| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit());
            if !digits(int) || frac.is_some_and(|f| !digits(f) || f.len() > 9) {
                return Err(Invalid);
            }
            // Units strictly in order within their section.
            let after = parts
                .last()
                .map_or(0, |(i, _, _): &(usize, &str, Option<&str>)| i + 1);
            let idx = (after..UNITS.len())
                .find(|&i| UNITS[i].0 == unit && UNITS[i].2 == in_time)
                .ok_or(Invalid)?;
            parts.push((idx, int, frac));
            rest = &rest[end + 1..];
        }
    }
    let week = parts.iter().any(|(i, _, _)| *i == 2);
    let fraction_not_last = parts.iter().rev().skip(1).any(|(_, _, f)| f.is_some());
    if parts.is_empty() || (week && parts.len() > 1) || fraction_not_last {
        return Err(Invalid);
    }
    if parts.iter().any(|(i, _, _)| *i < 2) {
        return Err(Rejected::CalendarUnit);
    }
    let mut nanos: u128 = 0;
    for (i, int, frac) in parts {
        let unit = u128::from(UNITS[i].1) * NANOS_PER_SEC;
        let whole: u128 = int.parse::<u64>().map_err(|_| Invalid)?.into();
        nanos += whole * unit;
        if let Some(f) = frac {
            let scale = 10u128.pow(f.len() as u32);
            nanos += f.parse::<u128>().map_err(|_| Invalid)? * unit / scale;
        }
    }
    let secs = u64::try_from(nanos / NANOS_PER_SEC).map_err(|_| Invalid)?;
    Ok(Duration::new(secs, (nanos % NANOS_PER_SEC) as u32))
}

/// Seconds as a JSON number: an integer when whole, else a float.
#[cfg(feature = "openapi")]
fn secs_json(d: Duration) -> Value {
    if d.subsec_nanos() == 0 {
        d.as_secs().into()
    } else {
        d.as_secs_f64().into()
    }
}

/// Schema for `std::time::Duration`. Created via [`vld::duration()`](crate::duration).
///
/// Accepts a non-negative JSON number of seconds (`3`, `1.5`) or a string:
/// `10s` / `1.5s`, `250ms` (integer milliseconds) or an ISO 8601 duration
/// (RFC 3339 grammar): `PT15S`, `PT1.5S`, `P3DT4H30M`, `PT1H`, `P2W`.
///
/// ISO durations take weeks alone (`P2W`), or days and a time part `T` with
/// hours, minutes and seconds, each optional but in that order; only the last
/// number may have a fraction (`PT1.5H`, `P0,5D`; `.` or `,`, up to 9 digits).
/// A day is 86 400 s (no daylight-saving shifts). Years and months (`P1Y`,
/// `P2M`, `P1Y2M3D`) are not a fixed length, so they fail with
/// `invalid_format` (`duration_calendar_unit`) — use a date library for
/// calendar arithmetic.
///
/// Negative numbers fail with `too_small` (origin `duration`, minimum `0`);
/// anything else with `invalid_type`.
///
/// # Example
/// ```
/// use vld::prelude::*;
/// use std::time::Duration;
///
/// let d = vld::duration().max_secs(60);
/// assert_eq!(d.parse("1.5").unwrap(), Duration::from_millis(1500));
/// assert_eq!(d.parse(r#""PT2.25S""#).unwrap(), Duration::from_millis(2250));
/// assert_eq!(d.parse(r#""250ms""#).unwrap(), Duration::from_millis(250));
/// assert!(d.parse("-1").is_err());
///
/// let long = vld::duration();
/// assert_eq!(long.parse(r#""P1DT2H30M""#).unwrap(), Duration::from_secs(95_400));
/// let err = long.parse(r#""P1M""#).unwrap_err();
/// assert_eq!(err.issues[0].code.key(), "invalid_format");
/// ```
#[derive(Clone)]
pub struct ZDuration {
    min: Option<Duration>,
    max: Option<Duration>,
    custom_type_error: Option<String>,
}

impl ZDuration {
    /// Create the schema (the same as its `vld::` constructor function).
    pub fn new() -> Self {
        Self {
            min: None,
            max: None,
            custom_type_error: None,
        }
    }

    /// Set a custom error message for type mismatch.
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    /// Must be at least `d`.
    pub fn min(mut self, d: Duration) -> Self {
        self.min = Some(d);
        self
    }

    /// Must be at most `d`.
    pub fn max(mut self, d: Duration) -> Self {
        self.max = Some(d);
        self
    }

    /// Must be at least `secs` seconds.
    pub fn min_secs(self, secs: u64) -> Self {
        self.min(Duration::from_secs(secs))
    }

    /// Must be at most `secs` seconds.
    pub fn max_secs(self, secs: u64) -> Self {
        self.max(Duration::from_secs(secs))
    }
}

impl Default for ZDuration {
    fn default() -> Self {
        Self::new()
    }
}

impl VldSchema for ZDuration {
    type Output = Duration;

    fn parse_value(&self, value: &Value) -> Result<Duration, VldError> {
        let d = match value {
            Value::Number(n) => match (n.as_u64(), n.as_f64()) {
                (Some(secs), _) => Some(Duration::from_secs(secs)),
                (None, Some(f)) if f < 0.0 => {
                    return Err(VldError::from_code_with_value(
                        IssueCode::TooSmall {
                            origin: SizeOrigin::Duration,
                            minimum: 0.into(),
                            inclusive: true,
                        },
                        value,
                    ));
                }
                (None, Some(f)) => Duration::try_from_secs_f64(f).ok(),
                _ => None,
            },
            Value::String(s) => match parse_duration_string(s) {
                Ok(d) => Some(d),
                Err(Rejected::CalendarUnit) => {
                    return Err(VldError::from_code_with_value(
                        IssueCode::InvalidFormat {
                            format: FormatKind::DurationCalendarUnit,
                        },
                        value,
                    ));
                }
                Err(Rejected::Invalid) => None,
            },
            _ => None,
        }
        .ok_or_else(|| crate::primitives::type_error(&self.custom_type_error, "duration", value))?;

        if let Some(min) = self.min {
            if d < min {
                return Err(VldError::from_code_with_value(
                    IssueCode::TooSmall {
                        origin: SizeOrigin::Duration,
                        minimum: min.as_secs_f64().into(),
                        inclusive: true,
                    },
                    value,
                ));
            }
        }
        if let Some(max) = self.max {
            if d > max {
                return Err(VldError::from_code_with_value(
                    IssueCode::TooBig {
                        origin: SizeOrigin::Duration,
                        maximum: max.as_secs_f64().into(),
                        inclusive: true,
                    },
                    value,
                ));
            }
        }
        Ok(d)
    }
}

/// JSON Schema. `min`/`max` become `minimum`/`maximum` (seconds) on the
/// number branch and `x-minSeconds`/`x-maxSeconds` on the string branch.
#[cfg(feature = "openapi")]
impl crate::openapi::JsonSchema for ZDuration {
    fn json_schema(&self) -> serde_json::Value {
        let mut schema = serde_json::json!({
            "anyOf": [
                { "type": "string", "description": "Duration like `P3DT4H30M`, `PT15S`, `1.5s`, `250ms`" },
                { "type": "number", "minimum": 0, "description": "Duration in seconds" }
            ]
        });
        if let Some(min) = self.min {
            schema["anyOf"][0]["x-minSeconds"] = serde_json::json!(min.as_secs_f64());
            schema["anyOf"][1]["minimum"] = secs_json(min);
        }
        if let Some(max) = self.max {
            schema["anyOf"][0]["x-maxSeconds"] = serde_json::json!(max.as_secs_f64());
            schema["anyOf"][1]["maximum"] = secs_json(max);
        }
        schema
    }
}

#[cfg(feature = "openapi")]
impl crate::openapi::CollectNestedSchemas for ZDuration {}
