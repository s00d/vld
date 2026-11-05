//! Regression tests for the 0.5.0 audit (primitives / collections / combinators).

use serde_json::json;
use vld::prelude::*;

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("vld-audit-{}-{}", tag, unique));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// --- 1, 6: ISO parsing is char-boundary safe and calendar-correct -----------

#[test]
fn iso_non_ascii_does_not_panic() {
    for s in ["123é56789", "1é:00", "2024-01-01T", "2024-01-01Té", "é"] {
        let v = json!(s);
        assert!(vld::string().iso_date().parse_value(&v).is_err());
        assert!(vld::string().iso_time().parse_value(&v).is_err());
        assert!(vld::string().iso_datetime().parse_value(&v).is_err());
    }
}

#[test]
fn iso_date_checks_days_in_month_and_timezone() {
    let d = vld::string().iso_date();
    assert!(d.parse(r#""2024-02-29""#).is_ok());
    assert!(d.parse(r#""2023-02-29""#).is_err());
    assert!(d.parse(r#""2024-02-31""#).is_err());
    let dt = vld::string().iso_datetime();
    assert!(dt.parse(r#""2024-01-01T10:00:00+03:00""#).is_ok());
    assert!(dt.parse(r#""2024-01-01T10:00+garbage""#).is_err());
    assert!(dt.parse(r#""2024-01-01T10:00:00Zjunk""#).is_err());
}

// --- 2: ZPath::within cannot be escaped with `..` ---------------------------

#[test]
fn path_within_rejects_dotdot_escape_for_missing_paths() {
    let base = temp_dir("within");
    let schema = vld::path().within(&base);
    let escape = format!("{}/../etc/x", base.display());
    assert!(schema.parse_value(&json!(escape)).is_err());
    assert!(schema.parse_value(&json!("../../etc/passwd")).is_err());
    // Relative input is returned joined onto the base.
    let ok = schema.parse_value(&json!("sub/new.txt")).unwrap();
    assert_eq!(ok, base.join("sub/new.txt"));
    std::fs::remove_dir_all(&base).ok();
}

// --- 3: url_strict -----------------------------------------------------------

#[cfg(feature = "string-advanced")]
#[test]
fn url_strict_blocks_local_hosts() {
    let s = vld::string().url_strict();
    for bad in [
        "http://[::1]/",
        "http://0.0.0.0/",
        "http://localhost./",
        "http://LOCALHOST/",
        "http://[::ffff:192.168.0.1]/",
        "http://[::ffff:127.0.0.1]/",
        "http://[fe80::1]/",
        "http://10.0.0.1/",
    ] {
        assert!(s.parse_value(&json!(bad)).is_err(), "{}", bad);
    }
    assert!(s.parse(r#""https://example.com/a""#).is_ok());
    assert!(s.parse(r#""http://[2606:4700::1111]/""#).is_ok());
}

// --- 4: IP / CIDR via std parsers ------------------------------------------

#[test]
fn ip_and_cidr_are_strict() {
    assert!(vld::string().ipv4().parse(r#""1.2.3.+4""#).is_err());
    assert!(vld::string().ipv4().parse(r#""01.2.3.4""#).is_err());
    assert!(vld::string().ipv6().parse(r#""::ffff:1.2.3.4""#).is_ok());
    assert!(vld::string().ip().parse(r#""::ffff:1.2.3.4""#).is_ok());
    let cidr = vld::string().cidr();
    assert!(cidr.parse(r#""10.0.0.0/+8""#).is_err());
    assert!(cidr.parse(r#""10.0.0.0/08""#).is_err());
    assert!(cidr.parse(r#""10.0.0.0/33""#).is_err());
    assert!(cidr.parse(r#""10.0.0.0/8""#).is_ok());
    assert!(cidr.parse(r#""2001:db8::/32""#).is_ok());
}

// --- 5: ZFile within + bounded read ----------------------------------------

#[cfg(feature = "file")]
#[test]
fn file_within_and_size_limit() {
    let base = temp_dir("file");
    let f = base.join("data.txt");
    std::fs::write(&f, b"0123456789").unwrap();

    let schema = vld::file().within(&base);
    assert!(schema.parse_value(&json!({"path": "/etc/passwd"})).is_err());
    assert!(schema.parse_value(&json!("../outside.txt")).is_err());
    let ok = schema.parse_value(&json!("data.txt")).unwrap();
    assert_eq!(ok.path(), f.as_path());
    assert_eq!(ok.bytes(), Some(&b"0123456789"[..]));

    let small = vld::file().max_size(5);
    let err = small.parse_path(&f).unwrap_err();
    assert!(err.issues[0].message.contains("at most 5"));
    std::fs::remove_dir_all(&base).ok();
}

// --- 7, 8, 9: numbers -------------------------------------------------------

#[test]
fn int_is_exact_and_range_checked() {
    let s = vld::number().int();
    assert_eq!(s.parse("9007199254740993").unwrap(), 9007199254740993);
    assert!(s.parse("18446744073709551615").is_err());
    assert!(s.parse("1e300").is_err());
    let bounded = vld::number().int().max(9007199254740992);
    assert!(bounded.parse("9007199254740993").is_err());
}

#[test]
fn coerce_rejects_nan_and_infinity() {
    let s = vld::number().coerce();
    for bad in [r#""NaN""#, r#""inf""#, r#""-infinity""#] {
        assert!(s.parse(bad).is_err(), "{}", bad);
    }
    assert_eq!(s.parse(r#""2.5""#).unwrap(), 2.5);
}

#[test]
fn multiple_of_float_tolerance_and_zero() {
    let s = vld::number().multiple_of(0.1);
    assert!(s.parse("0.3").is_ok());
    assert!(s.parse("-0.3").is_ok());
    assert!(s.parse("0.35").is_err());
    assert!(vld::number().multiple_of(0.0).parse("5").is_err());
    assert!(vld::number().int().multiple_of(0).parse("5").is_err());
}

// --- 11: arrays -------------------------------------------------------------

#[test]
fn array_contains_and_unique() {
    let opt = vld::array(vld::any()).contains(1).min_contains(0);
    assert!(opt.parse("[]").is_ok());

    let u = vld::array(vld::any()).unique();
    let err = u.parse("[1, 2, 1.0, 2, 3]").unwrap_err();
    let paths: Vec<String> = err
        .issues
        .iter()
        .map(|i| i.path.iter().map(|p| p.to_string()).collect())
        .collect();
    assert_eq!(paths, vec!["[2]".to_string(), "[3]".to_string()]);
    assert!(u.parse(r#"[{"a":1,"b":2},{"b":2,"a":1.0}]"#).is_err());
}

#[cfg(feature = "openapi")]
#[test]
fn array_contains_schema_uses_const() {
    use vld::openapi::JsonSchema;
    let js = vld::array(vld::number()).contains(5).json_schema();
    assert_eq!(js["contains"], json!({"const": 5}));
    let js = vld::array(vld::number()).min_contains(2).json_schema();
    assert!(js.get("minContains").is_none());
}

// --- 12: literals compare numerically ---------------------------------------

#[test]
fn literal_numbers_compare_by_value() {
    assert_eq!(vld::literal(1.0).parse("1").unwrap(), 1.0);
    assert_eq!(vld::literal(5i64).parse("5.0").unwrap(), 5);
    assert!(vld::literal(5i64).parse("5.5").is_err());
}

// --- 13, 15, 24: decimal ----------------------------------------------------

#[cfg(feature = "decimal")]
#[test]
fn decimal_scientific_bounds_and_new_checks() {
    let d = vld::decimal();
    assert_eq!(d.parse("1e-7").unwrap().to_string(), "0.0000001");
    assert!(d.parse("1.5e20").is_ok());

    let err = vld::decimal().min("0.5").parse("0.1").unwrap_err();
    match &err.issues[0].code {
        IssueCode::TooSmall { minimum, .. } => assert_eq!(*minimum, 0.5),
        other => panic!("unexpected code {:?}", other),
    }
    let s = vld::decimal().gt("1").lt("2").multiple_of("0.25");
    assert!(s.parse(r#""1.5""#).is_ok());
    assert!(s.parse(r#""1""#).is_err());
    assert!(s.parse(r#""2""#).is_err());
    assert!(s.parse(r#""1.3""#).is_err());
}

// --- 14: bytes --------------------------------------------------------------

#[test]
fn bytes_bad_element_has_index_path() {
    let err = vld::bytes().parse(r#"[1, 300]"#).unwrap_err();
    assert_eq!(err.issues[0].path.len(), 1);
    assert_eq!(err.issues[0].path[0].to_string(), "[1]");
}

#[cfg(feature = "openapi")]
#[test]
fn bytes_string_schema_uses_encoded_length() {
    use vld::openapi::JsonSchema;
    let js = vld::bytes().base64().len(4).json_schema();
    assert_eq!(js["minLength"], 8);
    assert_eq!(js["maxLength"], 8);
    let js = vld::bytes().hex().min_len(3).json_schema();
    assert_eq!(js["minLength"], 6);
}

// --- 15: i18n exclusive bounds ----------------------------------------------

#[test]
fn i18n_exclusive_bounds_say_greater_than() {
    let err = vld::number().positive().parse("0").unwrap_err();
    let tr = vld::i18n::translate_error(&err, &vld::i18n::english());
    assert_eq!(tr.issues[0].message, "Value must be greater than 0");
    let err = vld::number().min(3.0).parse("1").unwrap_err();
    let tr = vld::i18n::translate_error(&err, &vld::i18n::english());
    assert_eq!(tr.issues[0].message, "Value must be at least 3");
}

// --- 19: map duplicates -----------------------------------------------------

#[test]
fn map_duplicate_key_is_error() {
    let m = vld::map(vld::string(), vld::number());
    let err = m.parse(r#"[["a", 1], ["a", 2]]"#).unwrap_err();
    assert_eq!(err.issues[0].code.key(), "duplicate_map_key");
    assert_eq!(err.issues[0].path[0].to_string(), "[1]");
}

// --- 21: unions --------------------------------------------------------------

#[test]
fn union_error_keeps_variant_issues() {
    let u = vld::union(vld::string().min(5), vld::number().min(10.0));
    let err = u.parse("3").unwrap_err();
    assert_eq!(err.issues[0].code.key(), "invalid_union");
    assert_eq!(err.issues[0].received, Some(json!(3)));
    assert!(err.issues.len() >= 3);
    assert!(err
        .issues
        .iter()
        .any(|i| i.message.starts_with("[variant 2]")));
}

#[test]
fn discriminated_union_errors_point_at_discriminator() {
    let s = vld::discriminated_union("type")
        .variant_str("a", vld::object().field("type", vld::literal("a")));
    let err = s.parse(r#"{"type": "zzz"}"#).unwrap_err();
    assert_eq!(err.issues[0].path[0].to_string(), ".type");
    let err = s.parse(r#"{}"#).unwrap_err();
    assert_eq!(err.issues[0].path[0].to_string(), ".type");
}

// --- 22: intersection with an issue-less failure -----------------------------

struct SilentFail;
impl VldSchema for SilentFail {
    type Output = ();
    fn parse_value(&self, _: &serde_json::Value) -> Result<(), VldError> {
        Err(VldError::new())
    }
}

#[test]
fn intersection_and_tuple_do_not_panic_on_empty_errors() {
    assert!(vld::intersection(SilentFail, vld::any())
        .parse("1")
        .is_err());
    assert!((SilentFail, vld::any()).parse("[1, 2]").is_err());
}

// --- 23: format strictness ----------------------------------------------------

#[test]
fn string_formats_are_strict() {
    let email = vld::string().email();
    assert!(email.parse(r#""a..b@x.com""#).is_err());
    assert!(email.parse(r#"".a@x.com""#).is_err());
    assert!(email.parse(r#""a.@x.com""#).is_err());
    assert!(email.parse(r#""a.b@x.com""#).is_ok());

    let phone = vld::string().phone();
    assert!(phone.parse(r#""+1abc2345678""#).is_err());
    assert!(phone.parse(r#""+1 (234) 567-8900""#).is_ok());

    let cc = vld::string().credit_card();
    assert!(cc.parse(r#""hello 4111111111111111""#).is_err());
    assert!(cc.parse(r#""4111 1111 1111 1111""#).is_ok());

    let emoji = vld::string().emoji();
    assert!(emoji.parse(r#""hello😀""#).is_err());
    assert!(emoji.parse(r#""😀👍""#).is_ok());

    let cuid2 = vld::string().cuid2();
    assert!(cuid2.parse(r#""a""#).is_err());
    assert!(cuid2.parse(&format!("\"a{}\"", "b".repeat(32))).is_err());
    assert!(cuid2.parse(r#""tz4a98xxat96iws9zmbrgj3a""#).is_ok());
}

// --- 24: API consistency ------------------------------------------------------

#[test]
fn new_type_error_and_coerce_builders() {
    assert_eq!(vld::number().int().coerce().parse(r#""42""#).unwrap(), 42);
    assert_eq!(
        vld::number()
            .int()
            .coerce()
            .parse(r#""9007199254740993""#)
            .unwrap(),
        9007199254740993
    );
    let e = vld::enumeration(&["a"])
        .type_error("enum!")
        .parse("1")
        .unwrap_err();
    assert_eq!(e.issues[0].message, "enum!");
    let e = vld::boolean().type_error("bool!").parse("1").unwrap_err();
    assert_eq!(e.issues[0].message, "bool!");
    let e = vld::json_value()
        .object()
        .type_error("obj!")
        .parse("1")
        .unwrap_err();
    assert_eq!(e.issues[0].message, "obj!");
}

// --- 25: JSON Schema keeps constraints ---------------------------------------

#[cfg(feature = "openapi")]
#[test]
fn json_schema_emits_strictest_constraints() {
    use vld::openapi::JsonSchema;

    let js = vld::number().min(5.0).min(0.0).json_schema();
    assert_eq!(js["minimum"], 5.0);
    let js = vld::number().positive().json_schema();
    assert_eq!(js["exclusiveMinimum"], 0);
    assert!(js.get("minimum").is_none());
    let js = vld::number().int().non_positive().safe().json_schema();
    assert_eq!(js["maximum"], 0);
    assert_eq!(js["minimum"], -9007199254740991i64);

    let js = vld::string().min(2).min(4).max(10).max(8).json_schema();
    assert_eq!(js["minLength"], 4);
    assert_eq!(js["maxLength"], 8);
    let js = vld::string()
        .starts_with("a.b")
        .ends_with("z")
        .json_schema();
    assert_eq!(js["pattern"], "^a\\.b");
    assert_eq!(js["allOf"][0]["pattern"], "z$");
    let js = vld::string().email().lowercase().json_schema();
    assert_eq!(js["format"], "email");
    assert_eq!(js["allOf"][0]["format"], "lowercase");

    let js = vld::record(vld::number())
        .min_keys(1)
        .max_keys(3)
        .json_schema();
    assert_eq!(js["minProperties"], 1);
    assert_eq!(js["maxProperties"], 3);

    let js = vld::duration().min_secs(5).json_schema();
    assert_eq!(js["anyOf"][1]["minimum"], 5);
}

#[cfg(all(feature = "openapi", feature = "decimal"))]
#[test]
fn decimal_schema_emits_sign_flags() {
    use vld::openapi::JsonSchema;
    let js = vld::decimal().positive().json_schema();
    assert_eq!(js["x-exclusiveMinDecimal"], "0");
}

#[test]
fn socket_addr_message_mentions_hostnames() {
    let err = vld::socket_addr().parse(r#""example.com:80""#).unwrap_err();
    assert!(err.issues[0].message.contains("hostnames are not resolved"));
}
