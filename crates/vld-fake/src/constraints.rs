//! Constraint helpers: string checks (vld's own format predicates), date
//! arithmetic for `formatMinimum` / `formatMaximum` / `x-past` / `x-future`,
//! and flattening of `allOf` into one schema.

use serde_json::{Map, Value};
use vld::prelude::VldSchema;

// ───────────────────────────── string checks ─────────────────────────────

/// Every string constraint of a schema, checked with vld's own predicates.
pub(crate) struct StrCheck {
    min: Option<usize>,
    max: Option<usize>,
    regexes: Vec<vld::regex_lite::Regex>,
    formats: Vec<vld::primitives::ZString>,
    /// `uuid-vN` versions (string-advanced formats checked by shape here).
    uuid_versions: Vec<u8>,
    /// `(format, lo, hi)`: the value must parse as one of them within bounds.
    temporal: Vec<(String, Option<Instant>, Option<Instant>)>,
}

impl StrCheck {
    pub(crate) fn new(obj: &Map<String, Value>) -> Self {
        let (min, max) = crate::len_bounds(obj);
        let regexes = all_of_values(obj, "pattern")
            .iter()
            .filter_map(|p| vld::regex_lite::Regex::new(p).ok())
            .collect();
        let fmts = all_of_values(obj, "format");
        let uuid_versions = fmts
            .iter()
            .filter_map(|f| f.strip_prefix("uuid-v").and_then(|v| v.parse().ok()))
            .collect();
        let formats = fmts.iter().filter_map(|f| format_schema(f)).collect();
        let temporal = temporal_formats(obj)
            .into_iter()
            .map(|f| {
                let (lo, hi) = temporal_bounds(&f, obj);
                (f, lo, hi)
            })
            .collect();
        Self {
            min,
            max,
            regexes,
            formats,
            uuid_versions,
            temporal,
        }
    }

    pub(crate) fn ok(&self, s: &str) -> bool {
        crate::len_ok(s, self.min, self.max)
            && self.regexes.iter().all(|r| r.is_match(s))
            && self
                .formats
                .iter()
                .all(|f| f.parse_value(&Value::String(s.to_string())).is_ok())
            && self
                .uuid_versions
                .iter()
                .all(|v| s.as_bytes().get(14) == Some(&(b'0' + v)))
            && (self.temporal.is_empty()
                || self.temporal.iter().any(|(fmt, lo, hi)| {
                    instant(fmt, s)
                        .is_some_and(|t| lo.is_none_or(|l| t >= l) && hi.is_none_or(|h| t <= h))
                }))
    }
}

/// The date formats a string schema is bounded as: its `date` / `date-time` /
/// `time` format, or — for date bounds without a format (a pattern-only
/// branch, e.g. vld's naive datetime) — all three.
pub(crate) fn temporal_formats(obj: &Map<String, Value>) -> Vec<String> {
    let fmts = all_of_values(obj, "format");
    if let Some(f) = fmts
        .iter()
        .find(|f| matches!(f.as_str(), "date" | "date-time" | "time"))
    {
        return vec![f.clone()];
    }
    let bounded = ["formatMinimum", "formatMaximum", "x-past", "x-future"]
        .iter()
        .any(|k| obj.contains_key(*k));
    if fmts.is_empty() && bounded {
        return ["date-time", "date", "time"].map(String::from).to_vec();
    }
    vec![]
}

/// `key` from the schema plus every `allOf` fragment, in order.
pub(crate) fn all_of_values(obj: &Map<String, Value>, key: &str) -> Vec<String> {
    obj.get(key)
        .into_iter()
        .chain(
            obj.get("allOf")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|s| s.get(key)),
        )
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

/// vld's own check for a JSON Schema `format` name (string-advanced formats
/// fall back to the base check of the same family).
fn format_schema(fmt: &str) -> Option<vld::primitives::ZString> {
    let s = vld::string();
    Some(match fmt {
        "email" => s.email(),
        "uri" | "url" | "url-strict" => s.url(),
        "uuid" | "uuid-v1" | "uuid-v4" | "uuid-v7" => s.uuid(),
        "ip" => s.ip(),
        "ipv4" => s.ipv4(),
        "ipv6" => s.ipv6(),
        "cidr" => s.cidr(),
        "mac" | "mac-address" => s.mac(),
        "hostname" => s.hostname(),
        "slug" => s.slug(),
        "color" => s.color(),
        "currency-code" => s.currency_code(),
        "country-code" => s.country_code(),
        "locale" => s.locale(),
        "cron" => s.cron(),
        "hex" => s.hex(),
        "credit-card" => s.credit_card(),
        "phone" | "phone-e164" => s.phone(),
        "semver" | "semver-full" => s.semver(),
        "jwt" => s.jwt(),
        "ascii" => s.ascii(),
        "alpha" => s.alpha(),
        "alphanumeric" => s.alphanumeric(),
        "lowercase" => s.lowercase(),
        "uppercase" => s.uppercase(),
        "byte" => s.base64(),
        "base64url" => s.base64url(),
        "json" => s.json(),
        "date" => s.iso_date(),
        "date-time" => s.iso_datetime(),
        "time" => s.iso_time(),
        "cuid2" => s.cuid2(),
        "ulid" => s.ulid(),
        "nanoid" => s.nanoid(),
        "emoji" => s.emoji(),
        _ => return None,
    })
}

// ──────────────────────────────── dates ──────────────────────────────────

/// Seconds since the Unix epoch (or of the day, for `time`) plus nanoseconds.
pub(crate) type Instant = (i64, u32);

pub(crate) const DAY: i64 = 86_400;

/// `formatMinimum` / `formatMaximum` (inclusive) and `x-past` / `x-future`
/// (a two-day margin around "now", so a value stays valid for a while).
pub(crate) fn temporal_bounds(
    fmt: &str,
    obj: &Map<String, Value>,
) -> (Option<Instant>, Option<Instant>) {
    let bound = |k: &str| {
        obj.get(k)
            .and_then(Value::as_str)
            .and_then(|b| instant(fmt, b))
    };
    let (mut lo, mut hi) = (bound("formatMinimum"), bound("formatMaximum"));
    if fmt != "time" {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        if obj.get("x-past").and_then(Value::as_bool) == Some(true) {
            let p = (now - 2 * DAY, 0);
            hi = Some(hi.map_or(p, |h| h.min(p)));
        }
        if obj.get("x-future").and_then(Value::as_bool) == Some(true) {
            let f = (now + 2 * DAY, 0);
            lo = Some(lo.map_or(f, |l| l.max(f)));
        }
    }
    (lo, hi)
}

/// Parse a `date` / `date-time` / `time` string; a date-time without an
/// offset counts as UTC.
pub(crate) fn instant(fmt: &str, s: &str) -> Option<Instant> {
    if !s.is_ascii() {
        return None;
    }
    match fmt {
        "date" => Some((days(s)? * DAY, 0)),
        "time" => time_of_day(s),
        "date-time" => {
            let (date, rest) = s.split_at_checked(10)?;
            let rest = rest.strip_prefix(['T', 't', ' '])?;
            let (time, offset) = match rest.strip_suffix(['Z', 'z']) {
                Some(t) => (t, 0),
                None => match rest
                    .len()
                    .checked_sub(6)
                    .and_then(|i| rest.split_at_checked(i))
                {
                    Some((t, o)) if o.starts_with(['+', '-']) && o.as_bytes()[3] == b':' => {
                        let h: i64 = o[1..3].parse().ok()?;
                        let m: i64 = o[4..].parse().ok()?;
                        let sign = if o.starts_with('-') { -1 } else { 1 };
                        (t, sign * (h * 3600 + m * 60))
                    }
                    _ => (rest, 0),
                },
            };
            let (secs, nanos) = time_of_day(time)?;
            Some((days(date)? * DAY + secs - offset, nanos))
        }
        _ => None,
    }
}

/// Days since 1970-01-01 of a `YYYY-MM-DD` date.
fn days(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let y: i64 = s[..4].parse().ok()?;
    let m: i64 = s[5..7].parse().ok()?;
    let d: i64 = s[8..].parse().ok()?;
    // Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// `YYYY-MM-DD` of days since 1970-01-01 (Hinnant's civil_from_days).
pub(crate) fn civil(z: i64) -> String {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// `HH:MM:SS` of seconds of the day.
pub(crate) fn hms(secs: i64) -> String {
    format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
}

/// `HH:MM[:SS[.frac]]` → (seconds of day, nanoseconds).
fn time_of_day(s: &str) -> Option<Instant> {
    let mut parts = s.splitn(3, ':');
    let h: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let (sec, nanos) = match parts.next() {
        None => (0, 0),
        Some(rest) => {
            let (sec, frac) = rest.split_once('.').unwrap_or((rest, ""));
            let digits: String = frac.chars().chain(std::iter::repeat('0')).take(9).collect();
            (sec.parse().ok()?, digits.parse().ok()?)
        }
    };
    Some((h * 3600 + m * 60 + sec, nanos))
}

// ─────────────────────────────── values ──────────────────────────────────

/// JSON equality with numbers compared by value (`1 == 1.0`), like vld.
pub(crate) fn json_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x == y || x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(a, b)| json_eq(a, b))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| json_eq(v, w)))
        }
        _ => a == b,
    }
}

// ─────────────────────────────── allOf ───────────────────────────────────

/// Flatten `schemas` (each may carry its own `allOf`) into one schema:
/// numeric / length / count bounds take the strictest value, `multipleOf`
/// the least common multiple, `required` the union, `enum`s the
/// intersection, `properties` / `items` of several parts become an `allOf` of
/// them; further `pattern`s / `format`s go into an `allOf` of fragments
/// (what vld itself emits and the string generator reads).
pub(crate) fn merge(schemas: &[Value]) -> Map<String, Value> {
    let mut out = Map::new();
    let mut extra: Vec<Value> = Vec::new();
    let mut flat = Vec::new();
    flatten(schemas, &mut flat);
    for part in flat {
        for (k, v) in part {
            match k.as_str() {
                "minimum" | "exclusiveMinimum" | "minLength" | "minItems" | "minProperties"
                | "minContains" | "x-minUniqueItems" => keep(&mut out, k, v, |new, old| new > old),
                "maximum" | "exclusiveMaximum" | "maxLength" | "maxItems" | "maxProperties"
                | "maxContains" | "x-maxUniqueItems" => keep(&mut out, k, v, |new, old| new < old),
                "multipleOf" => {
                    let m = match out.get("multipleOf").and_then(Value::as_f64) {
                        Some(old) => v.as_f64().map_or(old, |new| lcm(old, new)),
                        None => v.as_f64().unwrap_or(1.0),
                    };
                    out.insert(k, number(m));
                }
                "pattern" | "format" if out.contains_key(&k) => {
                    if out.get(&k) != Some(&v) {
                        extra.push(Value::Object([(k, v)].into_iter().collect()));
                    }
                }
                "required" => {
                    let req = out
                        .entry("required")
                        .or_insert_with(|| Value::Array(vec![]));
                    if let (Value::Array(req), Value::Array(new)) = (req, v) {
                        for r in new {
                            if !req.contains(&r) {
                                req.push(r);
                            }
                        }
                    }
                }
                "enum" => {
                    let e = match (out.get("enum"), v) {
                        (Some(Value::Array(old)), Value::Array(new)) => Value::Array(
                            old.iter()
                                .filter(|o| new.iter().any(|n| json_eq(o, n)))
                                .cloned()
                                .collect(),
                        ),
                        (_, new) => new,
                    };
                    out.insert(k, e);
                }
                "type" => {
                    // integer ∩ number = integer
                    let keep_int = out.get("type").and_then(Value::as_str) == Some("integer")
                        && v.as_str() == Some("number");
                    if !keep_int {
                        out.insert(k, v);
                    }
                }
                "properties" => {
                    let props = out
                        .entry("properties")
                        .or_insert_with(|| Value::Object(Map::new()));
                    if let (Value::Object(props), Value::Object(new)) = (props, v) {
                        for (name, s) in new {
                            let merged = match props.remove(&name) {
                                Some(old) => serde_json::json!({ "allOf": [old, s] }),
                                None => s,
                            };
                            props.insert(name, merged);
                        }
                    }
                }
                "items" | "additionalProperties" | "propertyNames" | "contains"
                    if out.get(&k).is_some_and(Value::is_object) && v.is_object() =>
                {
                    let old = out.remove(&k).unwrap_or_default();
                    out.insert(k, serde_json::json!({ "allOf": [old, v] }));
                }
                "additionalProperties" | "items" if v == Value::Bool(false) => {
                    out.insert(k, v);
                }
                "uniqueItems" if v == Value::Bool(true) => {
                    out.insert(k, v);
                }
                "formatMinimum" | "formatMaximum" => {
                    let fmt = out
                        .get("format")
                        .and_then(Value::as_str)
                        .unwrap_or("date")
                        .to_string();
                    let key = |x: &Value| x.as_str().and_then(|s| instant(&fmt, s));
                    let stricter = match (out.get(&k).and_then(key), key(&v)) {
                        (Some(old), Some(new)) => {
                            if k == "formatMinimum" {
                                new > old
                            } else {
                                new < old
                            }
                        }
                        _ => true,
                    };
                    if stricter {
                        out.insert(k, v);
                    }
                }
                _ => {
                    out.entry(k).or_insert(v);
                }
            }
        }
    }
    if !extra.is_empty() {
        out.insert("allOf".into(), Value::Array(extra));
    }
    out
}

/// Every object of `schemas` with nested `allOf`s expanded (depth first).
fn flatten(schemas: &[Value], out: &mut Vec<Map<String, Value>>) {
    for s in schemas {
        let Some(obj) = s.as_object() else { continue };
        let mut own = obj.clone();
        let nested = own.remove("allOf");
        out.push(own);
        if let Some(Value::Array(parts)) = nested {
            flatten(&parts, out);
        }
    }
}

/// Insert `v` under `k` unless the present value is already at least as strict.
fn keep(out: &mut Map<String, Value>, k: String, v: Value, stricter: fn(f64, f64) -> bool) {
    let better = match (out.get(&k).and_then(Value::as_f64), v.as_f64()) {
        (Some(old), Some(new)) => stricter(new, old),
        _ => true,
    };
    if better {
        out.insert(k, v);
    }
}

/// Least common multiple of two positive steps (decimals scaled to integers).
fn lcm(a: f64, b: f64) -> f64 {
    let scale = 10f64.powi(decimals(a).max(decimals(b)) as i32);
    let (x, y) = ((a * scale).round() as i128, (b * scale).round() as i128);
    if x <= 0 || y <= 0 {
        return a.max(b);
    }
    let g = gcd(x, y);
    (x / g * y) as f64 / scale
}

fn gcd(a: i128, b: i128) -> i128 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

fn decimals(n: f64) -> usize {
    format!("{n}").split('.').nth(1).map_or(0, str::len).min(9)
}

/// A JSON number, integral when it has no fraction.
fn number(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < 9.0e15 {
        Value::from(n as i64)
    } else {
        Value::from(n)
    }
}
