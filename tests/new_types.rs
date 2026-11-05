//! Tests for the 0.5 schema types: sized numbers, serde/FromStr adapters,
//! enum_of, time of day, keyed/ordered records, null/never/char, new string
//! formats, 12-tuples and fractional durations.

use serde_json::json;
use std::collections::{BTreeMap, HashMap};
#[cfg(any(feature = "chrono", feature = "jiff", feature = "time"))]
use vld::error::DateKind;
use vld::error::{IssueCode, SizeOrigin, StringValidation};
use vld::prelude::*;

fn code<T: std::fmt::Debug>(r: Result<T, VldError>) -> IssueCode {
    r.unwrap_err().issues[0].code.clone()
}

// ---------------------------------------------------------------------------
// Sized integers
// ---------------------------------------------------------------------------

#[test]
fn sized_int_ranges() {
    assert_eq!(vld::u8().parse("255").unwrap(), 255u8);
    assert_eq!(
        code(vld::u8().parse("256")),
        IssueCode::TooBig {
            origin: SizeOrigin::Number,
            maximum: json!(255),
            inclusive: true
        }
    );
    assert_eq!(
        code(vld::u8().parse("-1")),
        IssueCode::TooSmall {
            origin: SizeOrigin::Number,
            minimum: json!(0),
            inclusive: true
        }
    );
    assert_eq!(vld::i8().parse("-128").unwrap(), i8::MIN);
    assert!(vld::i8().parse("128").is_err());
    assert_eq!(vld::i16().parse("-32768").unwrap(), i16::MIN);
    assert!(vld::u16().parse("65536").is_err());
    assert_eq!(vld::i32().parse("2147483647").unwrap(), i32::MAX);
    assert!(vld::i32().parse("2147483648").is_err());
    assert_eq!(vld::u32().parse("4294967295").unwrap(), u32::MAX);
    assert_eq!(vld::i64().parse("-9223372036854775808").unwrap(), i64::MIN);
    assert!(vld::i64().parse("9223372036854775808").is_err());
    assert_eq!(vld::usize().parse("7").unwrap(), 7usize);
    assert_eq!(vld::isize().parse("-7").unwrap(), -7isize);
    assert!(vld::usize().parse("-7").is_err());
}

#[test]
fn u64_above_i64_max_is_exact() {
    assert_eq!(vld::u64().parse("18446744073709551615").unwrap(), u64::MAX);
    assert_eq!(
        vld::u64().parse("9223372036854775808").unwrap(),
        9_223_372_036_854_775_808u64
    );
    let err = vld::u64().parse("-1").unwrap_err();
    assert!(err.issues[0].message.contains("at least 0"), "{err}");
    // Beyond u64: serde_json reads it as a float, still out of range.
    assert!(vld::u64().parse("18446744073709551616").is_err());
    // Bounds keep full precision.
    let big = vld::u64().min(u64::MAX - 1);
    assert!(big.parse("18446744073709551615").is_ok());
    assert!(big.parse("18446744073709551613").is_err());
}

#[test]
fn sized_int_floats_and_types() {
    assert_eq!(vld::i32().parse("2.0").unwrap(), 2);
    assert_eq!(code(vld::i32().parse("2.5")), IssueCode::NotInt);
    assert_eq!(code(vld::u8().parse("1e3")), code(vld::u8().parse("1000")));
    assert_eq!(code(vld::i16().parse(r#""5""#)).key(), "invalid_type");
    assert_eq!(code(vld::i16().parse("true")).key(), "invalid_type");
    let msg = vld::u8()
        .type_error("byte please")
        .parse("null")
        .unwrap_err();
    assert_eq!(msg.issues[0].message, "byte please");
    let msg = vld::u8().int_error("whole!").parse("1.5").unwrap_err();
    assert_eq!(msg.issues[0].message, "whole!");
}

#[test]
fn sized_int_coerce() {
    assert_eq!(vld::u16().coerce().parse(r#""443""#).unwrap(), 443);
    assert_eq!(vld::u8().coerce().parse("true").unwrap(), 1);
    assert_eq!(vld::i8().coerce().parse(r#""-3.0""#).unwrap(), -3);
    assert!(vld::u8().coerce().parse(r#""300""#).is_err());
    assert!(vld::u8().coerce().parse(r#""1.5""#).is_err());
    assert!(vld::u8().coerce().parse(r#""abc""#).is_err());
    assert_eq!(
        vld::u64()
            .coerce()
            .parse(r#""18446744073709551615""#)
            .unwrap(),
        u64::MAX
    );
}

#[test]
fn sized_int_builders() {
    let s = vld::i32().min(-5).max(5);
    assert!(s.parse("-5").is_ok() && s.parse("5").is_ok());
    assert!(s.parse("6").is_err() && s.parse("-6").is_err());
    let s = vld::i32().gt(0).lt(10);
    assert!(s.parse("0").is_err() && s.parse("10").is_err() && s.parse("9").is_ok());
    assert!(vld::i32().gte(1).lte(1).parse("1").is_ok());
    assert!(vld::i16().positive().parse("0").is_err());
    assert!(vld::i16().negative().parse("-1").is_ok());
    assert!(vld::i16().non_negative().parse("-1").is_err());
    assert!(vld::i16().non_positive().parse("0").is_ok());
    assert!(vld::i64().safe().parse("9007199254740992").is_err());
    assert!(vld::u32().multiple_of(5).parse("25").is_ok());
    assert_eq!(
        code(vld::u32().multiple_of(5).parse("26")),
        IssueCode::NotMultipleOf { divisor: json!(5) }
    );
    assert!(vld::u32().multiple_of(0).parse("0").is_err());
    let err = vld::u8()
        .max(10)
        .with_messages(|k| (k == "too_big").then(|| "too many".to_string()))
        .parse("11")
        .unwrap_err();
    assert_eq!(err.issues[0].message, "too many");
    // Type range is checked before user bounds: one issue only.
    assert_eq!(vld::u8().max(10).parse("300").unwrap_err().issues.len(), 1);
}

#[test]
fn number_int_is_unchanged() {
    let n: i64 = vld::number().int().parse("42").unwrap();
    assert_eq!(n, 42);
    let _: ZInt = vld::number().int();
    let _: ZInteger<i64> = vld::number().int().min(1);
    assert!(vld::number().int().parse("9223372036854775808").is_err());
}

// ---------------------------------------------------------------------------
// f32
// ---------------------------------------------------------------------------

#[test]
fn f32_range_and_bounds() {
    assert_eq!(vld::f32().parse("1.5").unwrap(), 1.5f32);
    assert_eq!(
        code(vld::f32().parse("1e39")),
        IssueCode::TooBig {
            origin: SizeOrigin::Number,
            maximum: json!(f32::MAX as f64),
            inclusive: true
        }
    );
    assert_eq!(code(vld::f32().parse("-1e39")).key(), "too_small");
    assert_eq!(
        vld::f32().parse_value(&json!(f32::MAX as f64)).unwrap(),
        f32::MAX
    );
    assert_eq!(vld::f32().parse("3.4028235e38").unwrap(), f32::MAX);
    assert_eq!(vld::f32().parse("-3.4028235e38").unwrap(), f32::MIN);
    // Bounds compare in f32 space: 0.1 passes min(0.1) / max(0.1).
    assert!(vld::f32().min(0.1).max(0.1).parse("0.1").is_ok());
    assert!(vld::f32().gt(0.0).parse("0").is_err());
    assert!(vld::f32().lt(1.0).parse("1").is_err());
    assert!(vld::f32().positive().parse("-0.5").is_err());
    assert!(vld::f32().coerce().parse(r#""2.5""#).is_ok());
    assert!(vld::f32().parse(r#""2.5""#).is_err());
    assert!(vld::f32().multiple_of(0.5).parse("2.5").is_ok());
    assert!(vld::f32().non_negative().non_positive().parse("0").is_ok());
    assert!(vld::f32().negative().parse("0").is_err());
    assert!(vld::f32().gte(1.0).lte(2.0).parse("1.5").is_ok());
    let err = vld::f32().type_error("float!").parse("null").unwrap_err();
    assert_eq!(err.issues[0].message, "float!");
}

// ---------------------------------------------------------------------------
// from_serde / from_str / enum_of
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum Role {
    Admin,
    User,
}

#[derive(Debug, PartialEq, serde::Deserialize)]
struct Point {
    x: i32,
    y: i32,
}

#[test]
fn from_serde_adapter() {
    let role = vld::from_serde::<Role>();
    assert_eq!(role.parse(r#""admin""#).unwrap(), Role::Admin);
    let err = role.parse(r#""root""#).unwrap_err();
    assert_eq!(err.issues[0].code, IssueCode::ParseError);
    assert!(err.issues[0].message.contains("unknown variant"));
    assert_eq!(err.issues[0].received, Some(json!("root")));

    let p = vld::from_serde::<Point>()
        .parse(r#"{"x": 1, "y": -2}"#)
        .unwrap();
    assert_eq!(p, Point { x: 1, y: -2 });
    let err = vld::from_serde::<Point>().parse(r#"{"x": 1}"#).unwrap_err();
    assert!(err.issues[0].message.contains("missing field"));

    let nested = vld::array(vld::from_serde::<Role>());
    let err = nested.parse(r#"["user", "x"]"#).unwrap_err();
    assert_eq!(err.issues[0].path, vec![PathSegment::Index(1)]);
}

#[test]
fn from_str_adapter() {
    use std::net::IpAddr;
    let ip = vld::from_str::<IpAddr>();
    assert!(ip.parse(r#""192.168.0.1""#).unwrap().is_ipv4());
    assert!(ip.parse(r#""::1""#).unwrap().is_ipv6());
    let err = ip.parse(r#""300.0.0.1""#).unwrap_err();
    assert_eq!(err.issues[0].code.key(), "invalid_format");
    assert_eq!(
        err.issues[0].message,
        "300.0.0.1".parse::<IpAddr>().unwrap_err().to_string()
    );
    assert_eq!(code(ip.parse("1")).key(), "invalid_type");
    let err = ip.clone().type_error("ip string").parse("1").unwrap_err();
    assert_eq!(err.issues[0].message, "ip string");

    assert_eq!(vld::from_str::<u32>().parse(r#""42""#).unwrap(), 42);
    assert!(vld::from_str::<u32>().parse(r#""-1""#).is_err());
}

#[cfg(feature = "string-advanced")]
#[test]
fn from_str_third_party_types() {
    let id = vld::from_str::<uuid::Uuid>()
        .parse(r#""67e55044-10b1-426f-9247-bb680e5fe0c8""#)
        .unwrap();
    assert_eq!(id.get_version_num(), 4);
    assert!(vld::from_str::<uuid::Uuid>().parse(r#""nope""#).is_err());
    let url = vld::from_str::<url::Url>()
        .parse(r#""https://example.com/a?b=1""#)
        .unwrap();
    assert_eq!(url.host_str(), Some("example.com"));
    let v = vld::from_str::<semver::Version>()
        .parse(r#""1.2.3-beta.1""#)
        .unwrap();
    assert_eq!(v.minor, 2);
    assert!(vld::from_str::<semver::Version>()
        .parse(r#""1.2""#)
        .is_err());
}

#[test]
fn enum_of_adapter() {
    #[derive(Debug, Clone, PartialEq)]
    enum Level {
        Low,
        High,
    }
    let level = vld::enum_of(&[("low", Level::Low), ("high", Level::High)]);
    assert_eq!(level.parse(r#""low""#).unwrap(), Level::Low);
    assert_eq!(
        code(level.parse(r#""mid""#)),
        IssueCode::InvalidEnumValue {
            options: vec!["low".into(), "high".into()]
        }
    );
    assert_eq!(code(level.parse("1")).key(), "invalid_type");
    let err = level.type_error("level!").parse("1").unwrap_err();
    assert_eq!(err.issues[0].message, "level!");
    // Empty enum rejects everything.
    let none: ZEnumOf<u8> = vld::enum_of(&[]);
    assert!(none.parse(r#""x""#).is_err());
}

// ---------------------------------------------------------------------------
// Time of day
// ---------------------------------------------------------------------------

#[cfg(any(feature = "chrono", feature = "jiff", feature = "time"))]
fn check_time_backend<B: vld::primitives::date::DateBackend>()
where
    B::Time: std::fmt::Debug,
{
    let t = vld::primitives::date::ZTime::<B>::new;
    assert!(t().parse(r#""00:00:00""#).is_ok());
    assert!(t().parse(r#""23:59:59""#).is_ok());
    assert!(t().parse(r#""12:30:45.123456789""#).is_ok());
    for bad in [
        "24:00:00",
        "12:60:00",
        "12:00:60",
        "12:00",
        "12:00:00Z",
        "12:00:00+01:00",
        "1:00:00",
        "12:00:00.",
        "12:00:00.1234567890",
        "12:00:00 ",
        "",
        "é",
    ] {
        let v = serde_json::Value::String(bad.into());
        assert_eq!(
            t().parse_value(&v).unwrap_err().issues[0].code,
            IssueCode::InvalidDate {
                kind: DateKind::Time
            },
            "{bad}"
        );
    }
    assert_eq!(code(t().parse("1200")).key(), "invalid_type");

    let s = t().min("08:00:00").max("17:30:00");
    assert!(s.parse(r#""08:00:00""#).is_ok());
    assert!(s.parse(r#""17:30:00""#).is_ok());
    let err = s.parse(r#""07:59:59.999""#).unwrap_err();
    assert_eq!(
        err.issues[0].code,
        IssueCode::TooSmall {
            origin: SizeOrigin::Time,
            minimum: json!("08:00:00"),
            inclusive: true
        }
    );
    assert_eq!(err.issues[0].message, "Time must be at or after 08:00:00");
    let err = s.parse(r#""17:30:00.5""#).unwrap_err();
    assert_eq!(err.issues[0].message, "Time must be at or before 17:30:00");

    let parsed = t().parse(r#""10:15:00""#).unwrap();
    let bounded = t().min_time(parsed.clone()).max_time(parsed);
    assert!(bounded.parse(r#""10:15:00""#).is_ok());
    assert!(bounded.parse(r#""10:15:01""#).is_err());
}

#[test]
#[should_panic(expected = "Invalid time literal")]
#[cfg(feature = "chrono")]
fn time_bad_literal_panics() {
    let _ = vld::chrono::time().min("25:00:00");
}

#[cfg(feature = "chrono")]
#[test]
fn chrono_time() {
    check_time_backend::<vld::chrono::Chrono>();
    let t = vld::chrono::time().parse(r#""09:05:01.5""#).unwrap();
    assert_eq!(
        t,
        chrono::NaiveTime::from_hms_milli_opt(9, 5, 1, 500).unwrap()
    );
}

#[cfg(feature = "jiff")]
#[test]
fn jiff_time() {
    check_time_backend::<vld::jiff::Jiff>();
    let t = vld::jiff::time().parse(r#""09:05:01""#).unwrap();
    assert_eq!(t, jiff::civil::time(9, 5, 1, 0));
}

#[cfg(feature = "time")]
#[test]
fn time_crate_time() {
    check_time_backend::<vld::time::Time>();
    let t = vld::time::time().parse(r#""09:05:01.25""#).unwrap();
    assert_eq!(t, time::Time::from_hms_milli(9, 5, 1, 250).unwrap());
    // Bound display is zero-padded HH:MM:SS for the `time` crate too.
    let s = vld::time::time().min_time(time::Time::from_hms(8, 0, 0).unwrap());
    let err = s.parse(r#""07:00:00""#).unwrap_err();
    assert!(err.issues[0].message.ends_with("08:00:00.0"), "{err}");
}

#[cfg(any(
    all(feature = "chrono", not(feature = "jiff"), not(feature = "time")),
    all(feature = "jiff", not(feature = "chrono"), not(feature = "time")),
    all(feature = "time", not(feature = "chrono"), not(feature = "jiff"))
))]
#[test]
fn time_of_day_shortcut() {
    let s: ZTime = vld::time_of_day().max("12:00:00");
    assert!(s.parse(r#""11:59:59""#).is_ok());
    assert!(s.parse(r#""12:00:01""#).is_err());
}

// ---------------------------------------------------------------------------
// Records
// ---------------------------------------------------------------------------

#[test]
fn record_with_key_schema() {
    let s = vld::record_with_keys(vld::from_str::<u32>(), vld::string());
    let m: HashMap<u32, String> = s.parse(r#"{"1": "a", "20": "b"}"#).unwrap();
    assert_eq!(m[&20], "b");

    let err = s.parse(r#"{"x": "a", "2": 3}"#).unwrap_err();
    let paths: Vec<_> = err.issues.iter().map(|i| i.path.clone()).collect();
    assert!(paths.contains(&vec![PathSegment::Field("x".into())]));
    assert!(paths.contains(&vec![PathSegment::Field("2".into())]));

    // Bad key and bad value under the same key: both reported.
    let err = s.parse(r#"{"x": 1}"#).unwrap_err();
    assert_eq!(err.issues.len(), 2);

    let uuid_keys = vld::record_with_keys(vld::string().uuid(), vld::boolean());
    assert!(uuid_keys
        .parse(r#"{"67e55044-10b1-426f-9247-bb680e5fe0c8": true}"#)
        .is_ok());
    let err = uuid_keys.parse(r#"{"nope": true}"#).unwrap_err();
    assert_eq!(
        err.issues[0].code,
        IssueCode::InvalidString {
            validation: StringValidation::Uuid
        }
    );

    let roles = vld::record_with_keys(vld::from_serde::<Role>(), vld::number().int())
        .min_keys(1)
        .max_keys(2);
    let m = roles.parse(r#"{"admin": 1, "user": 2}"#).unwrap();
    assert_eq!(m[&Role::User], 2);
    assert_eq!(code(roles.parse("{}")).key(), "too_small");
    assert_eq!(code(roles.parse("[]")).key(), "invalid_type");
}

#[test]
fn ordered_records() {
    let m: BTreeMap<String, f64> = vld::record(vld::number())
        .ordered()
        .parse(r#"{"b": 2, "a": 1, "c": 3}"#)
        .unwrap();
    assert_eq!(
        m.keys().map(String::as_str).collect::<Vec<_>>(),
        ["a", "b", "c"]
    );

    let s: ZOrderedRecord<ZBoolean, vld::collections::KeySchema<vld::primitives::ZFromStr<i32>>> =
        vld::record_with_keys(vld::from_str::<i32>(), vld::boolean())
            .max_keys(3)
            .ordered();
    let m = s.parse(r#"{"10": true, "-1": false, "2": true}"#).unwrap();
    assert_eq!(m.keys().copied().collect::<Vec<_>>(), [-1, 2, 10]);
    assert!(s
        .parse(r#"{"1": true, "2": true, "3": true, "4": true}"#)
        .is_err());

    // Default record output is unchanged.
    let _: HashMap<String, bool> = vld::record(vld::boolean()).parse("{}").unwrap();
}

// ---------------------------------------------------------------------------
// null / never / char
// ---------------------------------------------------------------------------

#[test]
fn null_never_char() {
    vld::null().parse("null").unwrap();
    assert_eq!(
        code(vld::null().parse("0")),
        IssueCode::InvalidType {
            expected: "null".into(),
            received: "number".into()
        }
    );
    let err = vld::null()
        .type_error("must be null")
        .parse("{}")
        .unwrap_err();
    assert_eq!(err.issues[0].message, "must be null");

    for input in ["null", "1", r#""x""#, "{}", "[]", "true"] {
        assert_eq!(code(vld::never().parse(input)).key(), "invalid_type");
    }
    let err = vld::never().parse(r#""x""#).unwrap_err();
    assert_eq!(err.issues[0].message, "Expected never, received string");
    assert!(vld::never().optional().parse("null").unwrap().is_none());

    assert_eq!(vld::char().parse(r#""a""#).unwrap(), 'a');
    assert_eq!(vld::char().parse(r#""日""#).unwrap(), '日');
    assert_eq!(vld::char().parse(r#""😀""#).unwrap(), '😀');
    // Two scalars (flag emoji / combining mark) are not one char.
    assert!(vld::char().parse(r#""🇺🇸""#).is_err());
    assert!(vld::char().parse("\"e\u{301}\"").is_err());
    assert_eq!(
        code(vld::char().parse(r#""""#)),
        IssueCode::InvalidLength {
            origin: SizeOrigin::String,
            expected: 1
        }
    );
    assert_eq!(code(vld::char().parse("1")).key(), "invalid_type");
    let err = vld::char().type_error("char!").parse("1").unwrap_err();
    assert_eq!(err.issues[0].message, "char!");
}

// ---------------------------------------------------------------------------
// String formats
// ---------------------------------------------------------------------------

#[test]
fn hostname_format() {
    let h = vld::string().hostname();
    let label63 = "a".repeat(63);
    for ok in [
        "localhost",
        "example.com",
        "a-b.c-d.io",
        "xn--80ak6aa92e.com",
        label63.as_str(),
    ] {
        assert!(h.parse_value(&json!(ok)).is_ok(), "{ok}");
    }
    let label64 = "a".repeat(64);
    let long = vec!["abcdefghi"; 26].join("."); // 259 chars
    for bad in [
        "",
        "-a.com",
        "a-.com",
        "a..com",
        "a_b.com",
        "a b",
        label64.as_str(),
        long.as_str(),
        "é.com",
    ] {
        assert_eq!(
            code(h.parse_value(&json!(bad))),
            IssueCode::InvalidString {
                validation: StringValidation::Hostname
            },
            "{bad}"
        );
    }
}

#[test]
fn base64url_format() {
    let b = vld::string().base64url();
    for ok in ["QQ", "QUI", "QUJD", "QQ==", "-_-_", "aGVsbG8gd29ybGQ"] {
        assert!(b.parse_value(&json!(ok)).is_ok(), "{ok}");
    }
    for bad in ["", "Q", "a+b/", "QQ=x", "QUJD=", "a b", "é"] {
        assert_eq!(
            code(b.parse_value(&json!(bad))),
            IssueCode::InvalidString {
                validation: StringValidation::Base64Url
            },
            "{bad}"
        );
    }
    let err = b.parse(r#""+""#).unwrap_err();
    assert_eq!(err.issues[0].message, "Invalid Base64URL string");
    let err = vld::string()
        .base64url_msg("b64u!")
        .parse(r#""+""#)
        .unwrap_err();
    assert_eq!(err.issues[0].message, "b64u!");
}

#[test]
fn cidr_format() {
    let c = vld::string().cidr();
    for ok in [
        "10.0.0.0/8",
        "0.0.0.0/0",
        "192.168.1.1/32",
        "::/0",
        "2001:db8::/32",
    ] {
        assert!(c.parse_value(&json!(ok)).is_ok(), "{ok}");
    }
    for bad in [
        "10.0.0.0",
        "10.0.0.0/33",
        "10.0.0.0/08",
        "::/129",
        "x/8",
        "10.0.0.0/",
    ] {
        assert!(c.parse_value(&json!(bad)).is_err(), "{bad}");
    }
}

#[test]
fn json_format() {
    let j = vld::string().json();
    for ok in [
        "{}",
        "[1,2]",
        r#"{"a": [true, null]}"#,
        "1",
        r#""s""#,
        " null ",
    ] {
        assert!(j.parse_value(&json!(ok)).is_ok(), "{ok}");
    }
    for bad in ["", "{", "{'a': 1}", "[1,]", "1 2", "undefined"] {
        assert_eq!(
            code(j.parse_value(&json!(bad))),
            IssueCode::InvalidString {
                validation: StringValidation::Json
            },
            "{bad}"
        );
    }
    let err = j.parse(r#""{""#).unwrap_err();
    assert_eq!(err.issues[0].message, "Invalid JSON string");
    let err = vld::string()
        .json()
        .with_messages(|k| (k == "invalid_json").then(|| "json!".into()))
        .parse(r#""{""#)
        .unwrap_err();
    assert_eq!(err.issues[0].message, "json!");
}

// ---------------------------------------------------------------------------
// Tuples
// ---------------------------------------------------------------------------

#[test]
fn tuple_of_twelve() {
    let n = || vld::number().int();
    let s = (
        n(),
        n(),
        n(),
        n(),
        n(),
        n(),
        n(),
        n(),
        n(),
        n(),
        n(),
        vld::string(),
    );
    let out = s.parse(r#"[1,2,3,4,5,6,7,8,9,10,11,"z"]"#).unwrap();
    assert_eq!(out.10, 11);
    assert_eq!(out.11, "z");
    assert_eq!(
        code(s.parse("[1,2,3]")),
        IssueCode::InvalidLength {
            origin: SizeOrigin::Tuple,
            expected: 12
        }
    );
    let err = s.parse(r#"[1,2,3,4,5,6,7,8,9,10,11,12]"#).unwrap_err();
    assert_eq!(err.issues[0].path, vec![PathSegment::Index(11)]);

    let seven = (n(), n(), n(), n(), n(), n(), vld::boolean());
    assert!(seven.parse("[1,2,3,4,5,6,true]").unwrap().6);
}

// ---------------------------------------------------------------------------
// Duration
// ---------------------------------------------------------------------------

#[test]
fn duration_fractional_seconds() {
    use std::time::Duration;
    let d = vld::duration();
    assert_eq!(d.parse("1.5").unwrap(), Duration::from_millis(1500));
    assert_eq!(d.parse("0").unwrap(), Duration::ZERO);
    assert_eq!(d.parse("3").unwrap(), Duration::from_secs(3));
    assert_eq!(d.parse(r#""1.5s""#).unwrap(), Duration::from_millis(1500));
    assert_eq!(d.parse(r#""PT0.25S""#).unwrap(), Duration::from_millis(250));
    assert_eq!(d.parse(r#""10s""#).unwrap(), Duration::from_secs(10));
    assert_eq!(d.parse(r#""250ms""#).unwrap(), Duration::from_millis(250));
    assert_eq!(
        code(d.parse("-1")),
        IssueCode::TooSmall {
            origin: SizeOrigin::Duration,
            minimum: json!(0),
            inclusive: true
        }
    );
    assert_eq!(code(d.parse("-0.5")).key(), "too_small");
    for bad in [
        r#""-1s""#,
        r#""+1s""#,
        r#""1e3s""#,
        r#""infs""#,
        r#""NaNs""#,
        r#""1.2.3s""#,
        r#"".s""#,
        r#""s""#,
        r#""1.5ms""#,
        r#""PTS""#,
        "true",
        "1e300",
    ] {
        assert_eq!(code(d.parse(bad)).key(), "invalid_type", "{bad}");
    }
    let bounded = vld::duration().min(Duration::from_millis(500)).max_secs(2);
    assert!(bounded.parse("0.5").is_ok());
    assert!(bounded.parse("0.499").is_err());
    assert!(bounded.parse("2.001").is_err());
}

// ---------------------------------------------------------------------------
// JSON Schema
// ---------------------------------------------------------------------------

#[cfg(feature = "openapi")]
mod schemas {
    use super::*;
    use vld::openapi::JsonSchema;

    #[test]
    fn sized_int_schema() {
        assert_eq!(
            vld::u8().json_schema(),
            json!({"type": "integer", "format": "uint8", "minimum": 0, "maximum": 255})
        );
        assert_eq!(
            vld::i16().min(-10).json_schema(),
            json!({"type": "integer", "format": "int16", "minimum": -10, "maximum": 32767})
        );
        assert_eq!(
            vld::u64().json_schema(),
            json!({"type": "integer", "format": "uint64", "minimum": 0, "maximum": u64::MAX})
        );
        assert_eq!(vld::i64().json_schema()["minimum"], json!(i64::MIN));
        assert_eq!(vld::i32().json_schema()["format"], "int32");
        assert_eq!(vld::u16().json_schema()["format"], "uint16");
        assert_eq!(vld::u32().json_schema()["format"], "uint32");
        assert_eq!(vld::i8().json_schema()["format"], "int8");
        assert_eq!(vld::usize().json_schema()["format"], "uint64");
        assert_eq!(vld::isize().json_schema()["format"], "int64");
        // Exclusive bound at the type edge wins.
        let js = vld::u8().positive().json_schema();
        assert_eq!(js["exclusiveMinimum"], 0);
        assert!(js.get("minimum").is_none());
        // safe() is looser than i32: the type range wins.
        let js = vld::i32().safe().lt(100).multiple_of(5).json_schema();
        assert_eq!(js["minimum"], i32::MIN);
        assert_eq!(js["exclusiveMaximum"], 100);
        assert!(js.get("maximum").is_none());
        assert_eq!(js["multipleOf"], 5);
        // number().int() keeps its 0.4 schema.
        assert_eq!(
            vld::number().int().json_schema(),
            json!({"type": "integer"})
        );
        assert_eq!(
            vld::number().int().min(1).json_schema(),
            json!({"type": "integer", "minimum": 1})
        );
    }

    #[test]
    fn f32_schema() {
        assert_eq!(
            vld::f32().min(0.5).json_schema(),
            json!({"type": "number", "format": "float", "minimum": 0.5})
        );
    }

    #[test]
    fn adapter_schemas() {
        assert_eq!(vld::from_serde::<Role>().json_schema(), json!({}));
        let custom = json!({"type": "string", "enum": ["admin", "user"]});
        assert_eq!(
            vld::from_serde::<Role>()
                .schema(custom.clone())
                .json_schema(),
            custom
        );
        assert_eq!(
            vld::from_str::<std::net::IpAddr>().json_schema(),
            json!({"type": "string"})
        );
        assert_eq!(
            vld::from_str::<std::net::IpAddr>()
                .schema(json!({"type": "string", "format": "ip"}))
                .json_schema()["format"],
            "ip"
        );
        assert_eq!(
            vld::enum_of(&[("a", 1), ("b", 2)]).json_schema(),
            json!({"type": "string", "enum": ["a", "b"]})
        );
    }

    #[test]
    fn misc_schemas() {
        assert_eq!(vld::null().json_schema(), json!({"type": "null"}));
        assert_eq!(vld::never().json_schema(), json!({"not": {}}));
        assert_eq!(
            vld::char().json_schema(),
            json!({"type": "string", "minLength": 1, "maxLength": 1})
        );
        assert_eq!(
            vld::string().base64url().json_schema()["format"],
            "base64url"
        );
        assert_eq!(vld::string().json().json_schema()["format"], "json");
        assert_eq!(vld::string().hostname().json_schema()["format"], "hostname");
        let t = (
            vld::boolean(),
            vld::boolean(),
            vld::boolean(),
            vld::boolean(),
            vld::boolean(),
            vld::boolean(),
            vld::boolean(),
            vld::boolean(),
            vld::boolean(),
            vld::boolean(),
            vld::boolean(),
            vld::string(),
        );
        let js = t.json_schema();
        assert_eq!(js["maxItems"], 12);
        assert_eq!(js["prefixItems"][11]["type"], "string");
        let d = vld::duration()
            .min(std::time::Duration::from_millis(1500))
            .max_secs(5)
            .json_schema();
        assert_eq!(d["anyOf"][1]["type"], "number");
        assert_eq!(d["anyOf"][1]["minimum"], 1.5);
        assert_eq!(d["anyOf"][1]["maximum"], 5);
    }

    #[test]
    fn record_schemas() {
        let js = vld::record(vld::number()).json_schema();
        assert!(js.get("propertyNames").is_none());
        assert_eq!(js, vld::record(vld::number()).ordered().json_schema());
        let js = vld::record_with_keys(vld::string().uuid(), vld::boolean())
            .min_keys(1)
            .json_schema();
        assert_eq!(
            js["propertyNames"],
            json!({"type": "string", "format": "uuid"})
        );
        assert_eq!(js["additionalProperties"], json!({"type": "boolean"}));
        assert_eq!(js["minProperties"], 1);
        let js = vld::record_with_keys(vld::enumeration(&["a", "b"]), vld::number())
            .ordered()
            .json_schema();
        assert_eq!(js["propertyNames"]["enum"], json!(["a", "b"]));
    }

    #[cfg(feature = "chrono")]
    #[test]
    fn time_schema() {
        assert_eq!(
            vld::chrono::time()
                .min("08:00:00")
                .max("18:00:00")
                .json_schema(),
            json!({"type": "string", "format": "time",
                   "formatMinimum": "08:00:00", "formatMaximum": "18:00:00"})
        );
    }
}
