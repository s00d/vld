//! Property tests with targeted generators.
//!
//! Format checks are fed *near-valid* inputs (right shape, values around the
//! limits, stray non-ASCII) and compared with a small oracle, instead of
//! generic JSON that almost never reaches the format validators.

use proptest::prelude::*;
use serde_json::{json, Value};
use std::collections::HashSet;
use vld::prelude::*;

// --- generators ----------------------------------------------------------------

/// A char that sometimes is non-ASCII (multi-byte) or a separator.
fn noise_char() -> impl Strategy<Value = char> {
    prop_oneof![
        Just('é'),
        Just('٤'),
        Just('😀'),
        Just('T'),
        Just(':'),
        Just('-'),
        Just(' '),
        any::<char>(),
    ]
}

/// Maybe mutate: insert a char, truncate, or append a char.
fn mutate(base: String) -> impl Strategy<Value = String> {
    let len = base.chars().count();
    prop_oneof![
        4 => Just(base.clone()),
        1 => (0..=len, noise_char()).prop_map({
            let base = base.clone();
            move |(at, c)| {
                let mut chars: Vec<char> = base.chars().collect();
                chars.insert(at, c);
                chars.into_iter().collect()
            }
        }),
        1 => (0..len.max(1)).prop_map({
            let base = base.clone();
            move |cut| base.chars().take(cut).collect()
        }),
        1 => noise_char().prop_map(move |c| format!("{base}{c}")),
    ]
}

fn is_leap(y: u32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: u32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(y) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Strict `YYYY-MM-DD` with a real day of month (independent oracle).
fn date_oracle(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    if !b
        .iter()
        .enumerate()
        .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
    {
        return false;
    }
    let num = |r: std::ops::Range<usize>| s[r].parse::<u32>().unwrap();
    let (y, m, d) = (num(0..4), num(5..7), num(8..10));
    y >= 1 && (1..=12).contains(&m) && d >= 1 && d <= days_in_month(y, m)
}

/// `(text, valid)`: `YYYY-MM-DD` with out-of-range parts, then maybe mutated.
fn near_date() -> impl Strategy<Value = (String, bool)> {
    (1u32..=9999, 0u32..=13, 0u32..=32)
        .prop_flat_map(|(y, m, d)| mutate(format!("{y:04}-{m:02}-{d:02}")))
        .prop_map(|s| {
            let ok = date_oracle(&s);
            (s, ok)
        })
}

fn near_time() -> impl Strategy<Value = (String, bool)> {
    (0u32..=25, 0u32..=61, proptest::option::of(0u32..=61)).prop_map(|(h, m, s)| match s {
        Some(s) => (format!("{h:02}:{m:02}:{s:02}"), h < 24 && m < 60 && s < 60),
        None => (format!("{h:02}:{m:02}"), h < 24 && m < 60),
    })
}

fn near_ipv4() -> impl Strategy<Value = String> {
    (
        prop::collection::vec(0u32..=300, 3..=5),
        proptest::bool::weighted(0.1),
    )
        .prop_map(|(octets, zero_pad)| {
            octets
                .iter()
                .map(|o| {
                    if zero_pad {
                        format!("0{o}")
                    } else {
                        o.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(".")
        })
}

fn near_email() -> impl Strategy<Value = String> {
    (
        "[a-z0-9._+é-]{0,8}",
        "@{0,2}",
        "[a-z0-9.é-]{0,8}",
        "(\\.[a-z]{0,3})?",
    )
        .prop_map(|(local, at, domain, tld)| format!("{local}{at}{domain}{tld}"))
}

fn near_url() -> impl Strategy<Value = String> {
    (
        prop_oneof![
            Just("http://"),
            Just("https://"),
            Just("ftp://"),
            Just("http:/"),
            Just("")
        ],
        "[a-z0-9.é-]{0,10}",
        "(/[a-zé0-9%?=&]{0,6}){0,2}",
    )
        .prop_map(|(scheme, host, path)| format!("{scheme}{host}{path}"))
}

// --- format oracles --------------------------------------------------------------

proptest! {
    #[test]
    fn iso_date_matches_calendar((s, valid) in near_date()) {
        let got = vld::string().iso_date().parse_value(&json!(s)).is_ok();
        prop_assert_eq!(got, valid, "{:?}", s);
    }

    #[test]
    fn iso_time_matches_clock((s, valid) in near_time()) {
        let got = vld::string().iso_time().parse_value(&json!(s)).is_ok();
        prop_assert_eq!(got, valid, "{:?}", s);
    }

    /// `iso_datetime` = valid date + `T` + valid time (+ optional `Z`).
    #[test]
    fn iso_datetime_composes_date_and_time(
        (date, date_ok) in near_date(),
        (time, time_ok) in near_time(),
        zone in prop_oneof![Just(""), Just("Z"), Just("+03:00")],
    ) {
        let s = format!("{date}T{time}{zone}");
        let got = vld::string().iso_datetime().parse_value(&json!(s)).is_ok();
        prop_assert_eq!(got, date_ok && time_ok, "{:?}", s);
    }

    /// Backend date parsers agree with the string check on near-valid input.
    #[cfg(any(feature = "chrono", feature = "jiff", feature = "time"))]
    #[test]
    fn backend_dates_match_iso_date((s, valid) in near_date()) {
        #[cfg(feature = "chrono")]
        prop_assert_eq!(vld::chrono::date().parse_value(&json!(s)).is_ok(), valid, "chrono {:?}", s);
        #[cfg(feature = "jiff")]
        prop_assert_eq!(vld::jiff::date().parse_value(&json!(s)).is_ok(), valid, "jiff {:?}", s);
        #[cfg(feature = "time")]
        prop_assert_eq!(vld::time::date().parse_value(&json!(s)).is_ok(), valid, "time {:?}", s);
    }

    #[test]
    fn ipv4_matches_std(s in near_ipv4()) {
        let want = s.parse::<std::net::Ipv4Addr>().is_ok();
        prop_assert_eq!(vld::string().ipv4().parse_value(&json!(s)).is_ok(), want, "{:?}", s);
        prop_assert_eq!(vld::string().ip().parse_value(&json!(s)).is_ok(), want, "{:?}", s);
    }

    #[test]
    fn ipv6_matches_std(addr in any::<std::net::Ipv6Addr>(), noise in proptest::option::of(noise_char())) {
        let mut s = addr.to_string();
        if let Some(c) = noise { s.push(c); }
        let want = s.parse::<std::net::Ipv6Addr>().is_ok();
        prop_assert_eq!(vld::string().ipv6().parse_value(&json!(s)).is_ok(), want, "{:?}", s);
    }

    #[test]
    fn cidr_prefix_range(addr in any::<std::net::Ipv4Addr>(), prefix in 0u32..=40) {
        let s = format!("{addr}/{prefix}");
        prop_assert_eq!(vld::string().cidr().parse_value(&json!(s)).is_ok(), prefix <= 32, "{:?}", s);
    }

    /// Accepted emails have one `@` with a non-empty local part and domain.
    #[test]
    fn email_invariants(s in near_email()) {
        if vld::string().email().parse_value(&json!(s)).is_ok() {
            let parts: Vec<&str> = s.split('@').collect();
            prop_assert_eq!(parts.len(), 2, "{:?}", s);
            prop_assert!(!parts[0].is_empty() && !parts[1].is_empty(), "{:?}", s);
            prop_assert!(!s.contains(".."), "{:?}", s);
            prop_assert!(!parts[0].starts_with('.') && !parts[0].ends_with('.'), "{:?}", s);
        }
    }

    /// Accepted URLs are http(s) with a host.
    #[test]
    fn url_invariants(s in near_url()) {
        if vld::string().url().parse_value(&json!(s)).is_ok() {
            let rest = s.strip_prefix("https://").or_else(|| s.strip_prefix("http://"));
            prop_assert!(rest.is_some_and(|r| !r.is_empty()), "{:?}", s);
        }
    }
}

// --- range invariants ----------------------------------------------------------------

proptest! {
    /// Accepts iff the length in chars is within `[min, max]`.
    #[test]
    fn string_length_in_chars(s in "\\PC{0,12}", min in 0usize..8, extra in 0usize..8) {
        let max = min + extra;
        let n = s.chars().count();
        let ok = vld::string().min(min).max(max).parse_value(&json!(s)).is_ok();
        prop_assert_eq!(ok, (min..=max).contains(&n), "{:?} len {}", s, n);
    }

    #[test]
    fn int_range(n in any::<i64>(), lo in any::<i64>(), hi in any::<i64>()) {
        let ok = vld::number().int().min(lo).max(hi).parse_value(&json!(n)).is_ok();
        prop_assert_eq!(ok, lo <= n && n <= hi);
    }

    #[test]
    fn int_exclusive_range(n in -50i64..50, lo in -50i64..50, hi in -50i64..50) {
        let ok = vld::number().int().gt(lo).lt(hi).parse_value(&json!(n)).is_ok();
        prop_assert_eq!(ok, lo < n && n < hi);
    }

    #[test]
    fn float_range(x in -1e6f64..1e6, lo in -1e6f64..1e6, hi in -1e6f64..1e6) {
        let ok = vld::number().min(lo).max(hi).parse_value(&json!(x)).is_ok();
        prop_assert_eq!(ok, lo <= x && x <= hi);
    }

    #[test]
    fn array_length(len in 0usize..12, min in 0usize..6, extra in 0usize..6) {
        let max = min + extra;
        let v = Value::Array(vec![json!(1); len]);
        let ok = vld::array(vld::any()).min_len(min).max_len(max).parse_value(&v).is_ok();
        prop_assert_eq!(ok, (min..=max).contains(&len));
    }

    #[test]
    fn unique_iff_no_duplicates(items in prop::collection::vec(0i64..6, 0..8)) {
        let distinct = items.iter().collect::<HashSet<_>>().len() == items.len();
        let ok = vld::array(vld::number().int()).unique().parse_value(&json!(items)).is_ok();
        prop_assert_eq!(ok, distinct);
    }

    #[test]
    fn set_deduplicates(items in prop::collection::vec(0i64..6, 0..10)) {
        let out = vld::set(vld::number().int()).parse_value(&json!(items)).unwrap();
        prop_assert_eq!(out, items.iter().copied().collect::<HashSet<_>>());
    }

    /// The `received` echo is a char-boundary prefix, at most 100 chars.
    #[test]
    fn received_is_truncated_on_char_boundaries(s in "\\PC{0,160}") {
        let err = vld::string().max(0).parse_value(&json!(s));
        if let Err(err) = err {
            let got = err.issues[0].received.as_ref().unwrap().as_str().unwrap().to_string();
            prop_assert!(got.chars().count() <= 100);
            let kept = got.strip_suffix("...").unwrap_or(&got);
            prop_assert!(s.starts_with(kept), "{:?} vs {:?}", got, s);
        }
    }
}

// --- panic freedom on arbitrary JSON (a few representative schemas) ----------------

fn arb_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|i| json!(i)),
        (-1e9f64..1e9).prop_map(|f| json!(f)),
        "\\PC{0,12}".prop_map(Value::String),
    ];
    leaf.prop_recursive(3, 48, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
            prop::collection::vec(("[a-z]{1,4}|type|kind", inner), 0..5)
                .prop_map(|kv| Value::Object(kv.into_iter().collect())),
        ]
    })
}

proptest! {
    #[test]
    fn composite_schemas_never_panic(v in arb_json()) {
        let obj = vld::object()
            .field("type", vld::string().min(1))
            .field_optional("kind", vld::union(vld::number().int(), vld::string().email()))
            .catchall(vld::array(vld::any()).max_len(3));
        let _ = obj.parse_value(&v);
        let du = vld::discriminated_union("type")
            .variant_str("a", vld::object().field("type", vld::literal("a")))
            .variant_str("b", vld::object().field("type", vld::literal("b")).strict());
        let _ = du.parse_value(&v);
        let _ = vld::record(vld::bytes().base64()).parse_value(&v);
        let _ = vld::map(vld::string(), vld::duration()).parse_value(&v);
        let _ = vld::json_value().object().require_key("type").max_depth(2).parse_value(&v);
    }

    /// Every string format, on arbitrary (often non-ASCII) text.
    #[test]
    fn string_formats_never_panic(s in "\\PC{0,40}") {
        let v = json!(s);
        let schemas = [
            vld::string().email(), vld::string().url(), vld::string().uuid(),
            vld::string().ipv4(), vld::string().ipv6(), vld::string().cidr(),
            vld::string().iso_date(), vld::string().iso_time(), vld::string().iso_datetime(),
            vld::string().hostname(), vld::string().credit_card(), vld::string().phone(),
            vld::string().semver(), vld::string().jwt(), vld::string().base64(),
            vld::string().color(), vld::string().cron(), vld::string().cuid2(),
            vld::string().ulid(), vld::string().emoji(), vld::string().mac(),
        ];
        for schema in schemas {
            let _ = schema.parse_value(&v);
        }
        let _ = vld::socket_addr().parse_value(&v);
        let _ = vld::duration().parse_value(&v);
        let _ = vld::bytes().hex().parse_value(&v);
    }
}
