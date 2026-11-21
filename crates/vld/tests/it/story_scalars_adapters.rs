//! Stories: scalar schemas a form or config uses (`boolean`, `literal`, `enumeration`, `any`,
//! `null`, `never`, `char`) and the adapters that reuse existing Rust types (`from_serde`,
//! `from_str`, `enum_of`).

use crate::common::*;

use serde_json::json;
use vld::error::{IssueCode, SizeOrigin};
use vld::prelude::*;

fn code<T: std::fmt::Debug>(r: Result<T, vld::error::VldError>) -> IssueCode {
    r.unwrap_err().issues[0].code.clone()
}

/// Given a settings form with a strict flag, a coercing flag, literals and a colour enum
/// When the form posts JSON booleans, legacy text / numbers, near-miss literals and enum values
/// Then a strict `boolean` accepts only JSON booleans (`invalid_type` with expected `boolean`
/// otherwise); `coerce()` reads `"true"` / `"1"` / `"false"` / `"0"` and numbers (zero is
/// false) and rejects other text and `null`; literals compare strings exactly and numbers by
/// value (`1` == `1.0`, `5i64` accepts `5.0`, every integer type and `f32`); an enumeration
/// reports `invalid_enum_value` with its options (also for non-ASCII members built with
/// `from_strings`) and `invalid_type` for non-strings; `any` returns every value unchanged;
/// `type_error` replaces the type message.
#[test]
fn settings_form_scalars() {
    let b = vld::boolean();
    assert!(b.parse_value(&json!(true)).unwrap());
    assert!(!b.parse_value(&json!(false)).unwrap());
    for bad in [json!("true"), json!(1), json!(null)] {
        assert!(
            matches!(code(b.parse_value(&bad)), IssueCode::InvalidType { expected, .. } if expected == "boolean"),
            "{bad}"
        );
    }
    let c = vld::boolean().coerce();
    for (input, want) in [
        (json!("true"), true),
        (json!("1"), true),
        (json!("false"), false),
        (json!("0"), false),
        (json!(0), false),
        (json!(0.0), false),
        (json!(2), true),
        (json!(-1), true),
    ] {
        assert_eq!(c.parse_value(&input).unwrap(), want, "{input}");
    }
    for bad in [
        json!("TRUE"),
        json!("yes"),
        json!(""),
        json!("١"),
        json!(null),
    ] {
        assert_eq!(
            issues(&c.parse_value(&bad).unwrap_err()),
            [(String::new(), "invalid_type".to_string())],
            "{bad}"
        );
    }
    let e = vld::boolean().type_error("bool!").parse_value(&json!(1));
    assert_eq!(e.unwrap_err().issues[0].message, "bool!");

    let l = vld::literal("admin");
    assert_eq!(l.parse_value(&json!("admin")).unwrap(), "admin");
    assert_eq!(l.expected_value(), &json!("admin"));
    assert_eq!(
        code(l.parse_value(&json!("Admin"))),
        IssueCode::InvalidLiteral {
            expected: json!("admin")
        }
    );
    let n = vld::literal(42i64);
    assert_eq!(n.parse_value(&json!(42)).unwrap(), 42);
    for bad in [json!(43), json!("42")] {
        assert_eq!(code(n.parse_value(&bad)).key(), "invalid_literal", "{bad}");
    }
    assert!(vld::literal(true).parse_value(&json!(true)).unwrap());
    assert_eq!(
        code(vld::literal(true).parse_value(&json!(false))).key(),
        "invalid_literal"
    );
    assert_eq!(vld::literal(1.0).parse("1").unwrap(), 1.0);
    assert_eq!(vld::literal(5i64).parse("5.0").unwrap(), 5);
    assert!(vld::literal(5i64).parse("5.5").is_err());
    // 2^63 as a float equals `i64::MAX as f64` but is outside `i64`.
    assert_eq!(
        code(vld::literal(i64::MAX).parse("9223372036854775808.0")).key(),
        "invalid_literal"
    );
    assert!(vld::literal(-3i64).parse("-4").is_err());
    assert_eq!(vld::literal(5u8).parse("5").unwrap(), 5u8);
    assert_eq!(vld::literal(5i32).parse("5.0").unwrap(), 5i32);
    assert_eq!(
        vld::literal(u64::MAX)
            .parse("18446744073709551615")
            .unwrap(),
        u64::MAX
    );
    assert!(vld::literal(5u16).parse("6").is_err());
    assert_eq!(vld::literal(-3isize).parse("-3").unwrap(), -3isize);
    assert_eq!(vld::literal(0.1f32).parse("0.1").unwrap(), 0.1f32);

    let colour = vld::enumeration(&["red", "green", "blue"]);
    assert_eq!(colour.parse_value(&json!("red")).unwrap(), "red");
    for bad in [json!("yellow"), json!("RED")] {
        assert_eq!(
            code(colour.parse_value(&bad)),
            IssueCode::InvalidEnumValue {
                options: vec!["red".into(), "green".into(), "blue".into()]
            }
        );
    }
    assert_eq!(code(colour.parse_value(&json!(42))).key(), "invalid_type");
    let accented = ZEnum::from_strings(vec!["é".to_string(), "b".to_string()]);
    assert_eq!(accented.parse_value(&json!("é")).unwrap(), "é");
    assert!(
        matches!(code(accented.parse_value(&json!("e"))), IssueCode::InvalidEnumValue { options } if options.len() == 2)
    );
    let e = vld::enumeration(&["a"]).type_error("enum!");
    assert_eq!(
        e.parse_value(&json!(1)).unwrap_err().issues[0].message,
        "enum!"
    );

    let a = vld::any();
    for v in [
        json!("hello"),
        json!(42),
        json!(null),
        json!([1, {"a": []}]),
    ] {
        assert_eq!(a.parse_value(&v).unwrap(), v);
    }
}

/// Given placeholders of an API: `null()` (must be null), `never()` (no value allowed) and
/// `char()` (one Unicode scalar)
/// When every kind of JSON value arrives
/// Then `null()` accepts only `null` (`invalid_type` expected `null`); `never()` rejects every
/// value as `invalid_type` (but `never().optional()` accepts `null` as `None`); `char()` accepts
/// one scalar (ASCII, CJK, emoji), rejects two scalars (flag, combining mark), reports `""` as
/// `invalid_length` 1 and a number as `invalid_type`; `type_error` replaces each type message.
#[test]
fn null_never_char_placeholders() {
    vld::null().parse("null").unwrap();
    assert_eq!(
        code(vld::null().parse("0")),
        IssueCode::InvalidType {
            expected: "null".into(),
            received: "number".into()
        }
    );
    let err = vld::null().type_error("must be null").parse("{}");
    assert_eq!(err.unwrap_err().issues[0].message, "must be null");

    for input in ["null", "1", r#""x""#, "{}", "[]", "true"] {
        assert_eq!(
            code(vld::never().parse(input)).key(),
            "invalid_type",
            "{input}"
        );
    }
    assert!(matches!(
        code(vld::never().parse(r#""x""#)),
        IssueCode::InvalidType { expected, received } if expected == "never" && received == "string"
    ));
    assert!(vld::never().optional().parse("null").unwrap().is_none());

    assert_eq!(vld::char().parse(r#""a""#).unwrap(), 'a');
    assert_eq!(vld::char().parse(r#""日""#).unwrap(), '日');
    assert_eq!(vld::char().parse(r#""😀""#).unwrap(), '😀');
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
    let err = vld::char().type_error("char!").parse("1");
    assert_eq!(err.unwrap_err().issues[0].message, "char!");
}

#[derive(Debug, PartialEq, serde::Deserialize)]
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

/// Given existing Rust types reused as schemas: a serde enum and struct (`from_serde`), `FromStr`
/// types (`from_str`: `IpAddr`, `u32`) and a value table (`enum_of`)
/// When good values, bad values and wrong JSON types are parsed, alone and inside an array
/// Then `from_serde` returns the value or a `parse_error` carrying serde's message and the
/// received value (at the element index inside an array); `from_str` returns the parsed value,
/// reports a bad string as `invalid_format` with the `FromStr` error text and a non-string as
/// `invalid_type`; `enum_of` maps names to values, reports `invalid_enum_value` with the names
/// and rejects everything when empty; `type_error` replaces the type message; a configured adapter
/// clones with its message and JSON Schema.
#[test]
fn reusing_rust_types() {
    let role = vld::from_serde::<Role>();
    assert_eq!(role.parse(r#""admin""#).unwrap(), Role::Admin);
    assert_eq!(role.parse(r#""user""#).unwrap(), Role::User);
    let err = role.parse(r#""root""#).unwrap_err();
    assert_eq!(err.issues[0].code, IssueCode::ParseError);
    assert!(err.issues[0].message.contains("unknown variant"));
    assert_eq!(err.issues[0].received, Some(json!("root")));
    assert_eq!(
        vld::from_serde::<Point>()
            .parse(r#"{"x": 1, "y": -2}"#)
            .unwrap(),
        Point { x: 1, y: -2 }
    );
    let err = vld::from_serde::<Point>().parse(r#"{"x": 1}"#).unwrap_err();
    assert!(err.issues[0].message.contains("missing field"));
    let err = vld::array(vld::from_serde::<Role>())
        .parse(r#"["user", "x"]"#)
        .unwrap_err();
    assert_eq!(
        issues(&err),
        [("[1]".to_string(), "parse_error".to_string())]
    );

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
    // A configured adapter is cloned with its settings.
    let typed = ip.clone().type_error("ip string");
    let err = typed.clone().parse("1").unwrap_err();
    assert_eq!(err.issues[0].message, "ip string");
    #[cfg(feature = "openapi")]
    {
        use vld::openapi::JsonSchema;
        let roles = json!({"enum": ["admin", "user"]});
        assert_eq!(role.schema(roles.clone()).clone().json_schema(), roles);
        let ipv4 = json!({"type": "string", "format": "ipv4"});
        assert_eq!(typed.schema(ipv4.clone()).clone().json_schema(), ipv4);
    }
    assert_eq!(vld::from_str::<u32>().parse(r#""42""#).unwrap(), 42);
    assert!(vld::from_str::<u32>().parse(r#""-1""#).is_err());

    #[derive(Debug, Clone, PartialEq)]
    enum Level {
        Low,
        High,
    }
    let level = vld::enum_of(&[("low", Level::Low), ("high", Level::High)]);
    assert_eq!(level.parse(r#""low""#).unwrap(), Level::Low);
    assert_eq!(level.parse(r#""high""#).unwrap(), Level::High);
    assert_eq!(
        code(level.parse(r#""mid""#)),
        IssueCode::InvalidEnumValue {
            options: vec!["low".into(), "high".into()]
        }
    );
    assert_eq!(code(level.parse("1")).key(), "invalid_type");
    let err = level.type_error("level!").parse("1").unwrap_err();
    assert_eq!(err.issues[0].message, "level!");
    let none: ZEnumOf<u8> = vld::enum_of(&[]);
    assert!(none.parse(r#""x""#).is_err());
}

/// Given third-party `FromStr` types behind `string-advanced` (`uuid::Uuid`, `url::Url`,
/// `semver::Version`)
/// When valid and invalid strings are parsed with `from_str`
/// Then the typed value comes back (version 4 UUID, URL host, semver minor) and bad strings fail.
#[cfg(feature = "string-advanced")]
#[test]
fn third_party_types_via_from_str() {
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
    assert!(
        vld::from_str::<semver::Version>()
            .parse(r#""1.2""#)
            .is_err()
    );
}

/// Given an API type with `uuid::Uuid` and `url::Url` fields
/// When ids and links are parsed through `vld::uuid()` / `vld::url()` and
/// through the types' inferred schemas
/// Then the outputs are typed values, bad input is the same issue as the
/// string format's, and the JSON Schema is `format: uuid` / `uri`.
#[cfg(feature = "string-advanced")]
#[test]
fn typed_uuid_and_url() {
    use vld::error::StringValidation;
    use vld::schema::VldType;
    let id = vld::uuid();
    let v = id
        .parse_value(&json!("550e8400-e29b-41d4-a716-446655440000"))
        .unwrap();
    assert_eq!(
        v,
        uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()
    );
    assert_eq!(
        code(id.parse_value(&json!("550e8400"))),
        IssueCode::InvalidString {
            validation: StringValidation::Uuid
        }
    );
    // Narrowed with string checks: only v7, case-folded first.
    let v7 = vld::uuid().checks(|s| s.to_lowercase().uuid_v7());
    assert!(
        v7.parse_value(&json!("01890F57-5A7B-7F8B-BFD3-63F8E7C6F4B8"))
            .is_ok()
    );
    assert_eq!(
        code(v7.parse_value(&json!("550e8400-e29b-41d4-a716-446655440000"))),
        IssueCode::InvalidString {
            validation: StringValidation::UuidV7
        }
    );

    let link = vld::url();
    let u = link
        .parse_value(&json!("https://example.com/a?b=1"))
        .unwrap();
    assert_eq!((u.scheme(), u.path()), ("https", "/a"));
    assert!(link.parse_value(&json!("mailto:a@b.c")).is_ok());
    assert_eq!(
        code(link.parse_value(&json!("/relative"))),
        IssueCode::InvalidString {
            validation: StringValidation::Uri
        }
    );
    let https = vld::url().checks(|s| s.url());
    assert!(https.parse_value(&json!("mailto:a@b.c")).is_err());

    // Inferred schemas (what `#[derive(Validate)]` uses for such fields).
    assert!(
        <uuid::Uuid>::vld_schema()
            .parse_value(&json!("00000000-0000-0000-0000-000000000000"))
            .is_ok()
    );
    let opt = <Option<url::Url>>::vld_schema();
    assert_eq!(opt.parse_value(&json!(null)).unwrap(), None);
    #[cfg(feature = "openapi")]
    {
        assert_eq!(
            id.json_schema(),
            json!({"type": "string", "format": "uuid"})
        );
        assert_eq!(
            link.json_schema(),
            json!({"type": "string", "format": "uri"})
        );
    }
}
