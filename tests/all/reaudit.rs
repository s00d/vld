//! Regression tests for the 0.5 re-audit findings (one test per finding,
//! numbered as in the audit).

use serde_json::json;
use vld::prelude::*;

// 1. `multiple_of(-1)` on a value saturated to `i128::MIN` overflowed `%`.
#[test]
fn f01_multiple_of_minus_one_never_panics() {
    let s = vld::i64().multiple_of(-1);
    assert!(s.parse_value(&json!(-1e40)).is_err());
    let s = vld::i64().multiple_of(-1).coerce();
    assert!(s
        .parse_value(&json!("-170141183460469231731687303715884105728"))
        .is_err());
    assert_eq!(s.parse_value(&json!(-7)).unwrap(), -7);
}

// 2 + 3. Range edges: no panic, and all date backends agree.
#[cfg(any(feature = "chrono", feature = "jiff", feature = "time"))]
macro_rules! edge_suite {
    ($backend:ident) => {
        mod $backend {
            use serde_json::json;
            use vld::prelude::*;

            #[test]
            fn f02_f03_datetime_range_edges_agree() {
                let s = vld::$backend::datetime();
                for bad in [
                    "9999-12-31T23:00:00-05:00",
                    "9999-12-31T00:00:00Z",
                    "9999-12-30T22:00:01Z",
                    "0000-01-01T00:30:00+01:00",
                ] {
                    assert!(s.parse_value(&json!(bad)).is_err(), "{bad}");
                }
                for ok in [
                    "9999-12-30T22:00:00.999999999Z",
                    "9999-12-31T00:00:00+02:00",
                    "0000-01-01T00:00:00Z",
                    "0000-01-01T00:30:00-01:00",
                ] {
                    assert!(s.parse_value(&json!(ok)).is_ok(), "{ok}");
                }
                // Naive input shifted past either end by the naive offset.
                let n = vld::$backend::datetime().naive_timezone_offset(-5 * 3600);
                assert!(n.parse_value(&json!("9999-12-31T23:00:00")).is_err());
                let n = vld::$backend::datetime().naive_timezone_offset(3600);
                assert!(n.parse_value(&json!("0000-01-01T00:00:00")).is_err());
            }
        }
    };
}
#[cfg(feature = "chrono")]
edge_suite!(chrono);
#[cfg(feature = "jiff")]
edge_suite!(jiff);
#[cfg(feature = "time")]
edge_suite!(time);

// 4. f32 bounds work at f32 precision and print the short form.
#[test]
fn f04_f32_multiple_of_and_messages() {
    let s = vld::f32().multiple_of(0.1);
    assert_eq!(s.parse_value(&json!(0.3)).unwrap(), 0.3f32);
    let err = s.parse_value(&json!(0.35)).unwrap_err();
    assert_eq!(err.issues[0].message, "Number must be a multiple of 0.1");
    assert_eq!(vld::f32().max(0.1).parse_value(&json!(0.1)).unwrap(), 0.1f32);
    let err = vld::f32().min(0.1).parse_value(&json!(0.05)).unwrap_err();
    assert_eq!(err.issues[0].message, "Number must be at least 0.1");
}

// 5. `union!` with 7 and 13 schemas compiles and keeps first-match order.
#[test]
fn f05_union_macro_7_and_13() {
    let s7 = vld::union!(
        vld::literal(1),
        vld::literal(2),
        vld::literal(3),
        vld::literal(4),
        vld::literal(5),
        vld::literal(6),
        vld::literal(7),
    );
    assert!(s7.parse("7").is_ok());
    assert!(s7.parse("8").is_err());
    let s13 = vld::union!(
        vld::literal(1),
        vld::literal(2),
        vld::literal(3),
        vld::literal(4),
        vld::literal(5),
        vld::literal(6),
        vld::literal(7),
        vld::literal(8),
        vld::literal(9),
        vld::literal(10),
        vld::literal(11),
        vld::literal(12),
        vld::literal(13)
    );
    assert!(s13.parse("13").is_ok());
    // 19 (unions): nested unions are flattened, prefixes never stack.
    let err = s13.parse("14").unwrap_err();
    assert_eq!(err.issues[0].code, IssueCode::InvalidUnion);
    let msgs: Vec<&str> = err.issues[1..].iter().map(|i| i.message.as_str()).collect();
    assert_eq!(msgs.len(), 13);
    for (i, m) in msgs.iter().enumerate() {
        assert_eq!(*m, format!("[variant {}] Expected literal {}", i + 1, i + 1));
    }
}

// 6. `schema!` honors serde renames.
vld::schema! {
    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct F06Camel {
        first_name: String => vld::string().min(1),
        #[serde(rename = "years")]
        age_years: i64 => vld::i64().min(0),
        #[serde(rename(serialize = "outName", deserialize = "inName"))]
        split_name: String => vld::string(),
    }
}

#[test]
fn f06_schema_macro_serde_rename() {
    let v = F06Camel::parse(
        r#"{"firstName": "A", "years": 3, "inName": "x"}"#,
    )
    .unwrap();
    assert_eq!(v.first_name, "A");
    // A serialized instance validates (serialize names).
    assert!(F06Camel::validate(&v).is_ok());
    assert_eq!(
        serde_json::to_value(&v).unwrap()["outName"],
        json!("x")
    );
    let err = F06Camel::parse(r#"{"first_name": "A", "years": 3, "inName": "x"}"#)
        .unwrap_err();
    assert_eq!(crate::common::paths(&err), vec![".firstName"]);
    #[cfg(feature = "openapi")]
    {
        let props = &F06Camel::json_schema()["properties"];
        for k in ["firstName", "years", "inName"] {
            assert!(props.get(k).is_some(), "{k}");
        }
    }
}

// 7. derive: validate() uses serialize names, parse uses deserialize names.
#[cfg(feature = "derive")]
mod f07 {
    use serde_json::json;

    #[derive(Debug, serde::Serialize, vld::Validate)]
    #[serde(rename_all(serialize = "camelCase", deserialize = "kebab-case"))]
    struct Split {
        #[serde(rename(serialize = "a", deserialize = "b"))]
        #[vld(vld::string().min(1))]
        field: String,
        #[vld(vld::string())]
        other_name: String,
    }

    #[test]
    fn f07_derive_split_rename() {
        let v = Split::parse_value(&json!({"b": "x", "other-name": "y"})).unwrap();
        assert!(Split::validate(&v).is_ok());
        assert!(Split::is_valid(&json!({"a": "x", "otherName": "y"})));
        assert!(!Split::is_valid(&json!({"b": "x", "other-name": "y"})));
    }
}

fn msg<S: VldSchema>(s: &S, input: serde_json::Value) -> String
where
    S::Output: std::fmt::Debug,
{
    crate::common::parse_err(s, input).issues[0].message.clone()
}

// 8. Translation keeps custom messages (and union prefixes).
#[test]
fn f08_translate_keeps_custom_messages() {
    use vld::i18n::{german, translate_error};
    let de = |e: &VldError| translate_error(e, &german()).issues[0].message.clone();
    let err = crate::common::parse_err(&vld::string().min_msg(3, "my custom"), json!("ab"));
    assert!(err.issues[0].has_custom_message());
    assert_eq!(de(&err), "my custom");
    let s = vld::string().min(3).with_messages(|_| Some("bulk".into()));
    assert_eq!(de(&crate::common::parse_err(&s, json!("ab"))), "bulk");
    let err = crate::common::parse_err(&vld::string().min(3), json!("ab"));
    assert!(!err.issues[0].has_custom_message());
    assert_eq!(
        de(&err),
        "Zeichenkette muss mindestens 3 Zeichen lang sein"
    );
    let u = vld::union(vld::string().min(3), vld::number());
    let t = translate_error(&crate::common::parse_err(&u, json!("ab")), &german());
    assert_eq!(
        t.issues[1].message,
        "[variant 1] Zeichenkette muss mindestens 3 Zeichen lang sein"
    );
}

// 19. Message wording: not_int, singular/plural, Russian invalid_type.
#[test]
fn f19_messages() {
    use vld::i18n::{russian, translate_error};
    assert_eq!(
        msg(&vld::i32().coerce(), json!("1.5")),
        "Expected integer"
    );
    assert_eq!(
        msg(&vld::char(), json!("ab")),
        "String must be exactly 1 character"
    );
    assert_eq!(
        msg(&vld::array(vld::string()).min_len(1), json!([])),
        "Array must have at least 1 element"
    );
    let ru = |s: &ZString, v: serde_json::Value| {
        translate_error(&crate::common::parse_err(s, v), &russian()).issues[0]
            .message
            .clone()
    };
    assert_eq!(
        ru(&vld::string().len(1), json!("")),
        "Строка должна содержать ровно 1 символ"
    );
    assert_eq!(
        ru(&vld::string().len(3), json!("")),
        "Строка должна содержать ровно 3 символа"
    );
    assert_eq!(
        ru(&vld::string().len(5), json!("")),
        "Строка должна содержать ровно 5 символов"
    );
    assert_eq!(
        ru(&vld::string().len(12), json!("")),
        "Строка должна содержать ровно 12 символов"
    );
    assert_eq!(
        ru(&vld::string().min(21), json!("")),
        "Строка должна содержать не менее 21 символа"
    );
    assert_eq!(
        ru(&vld::string(), json!(true)),
        "Ожидался тип «строка», получен «логическое значение»"
    );
}

// 20. invalid_type carries stable type keys; English renders the label.
#[test]
fn f20_type_keys() {
    let err = crate::common::parse_err(&vld::duration(), json!(true));
    assert_issue!(&err, "", IssueCode::InvalidType { expected, .. } if expected == "duration");
    assert!(err.issues[0].message.starts_with("Expected duration as seconds"));
    let t = vld::i18n::translate_error(&err, &vld::i18n::russian());
    assert_eq!(
        t.issues[0].message,
        "Ожидался тип «длительность (секунды или строка)», получен «логическое значение»"
    );
    let err = crate::common::parse_err(&vld::path(), json!(1));
    assert_issue!(&err, "", IssueCode::InvalidType { expected, .. } if expected == "path");
    assert_eq!(err.issues[0].message, "Expected path string, received number");
}

// 21. strict() / required() issues use templates (translatable).
#[test]
fn f21_object_templates() {
    use vld::i18n::{german, translate_error};
    let s = vld::object().field("a", vld::string()).strict();
    let err = crate::common::parse_err(&s, json!({"a": "x", "b": 1}));
    assert_eq!(crate::common::paths(&err), [".b"]);
    assert_eq!(
        translate_error(&err, &german()).issues[0].message,
        "Unbekanntes Feld"
    );
    let s = vld::object().field("a", vld::string()).partial().required();
    let err = crate::common::parse_err(&s, json!({"a": null}));
    assert_eq!(
        translate_error(&err, &german()).issues[0].message,
        "Pflichtfeld fehlt"
    );
}

// 28. with_messages covers checks added after it.
#[test]
fn f28_with_messages_order_independent() {
    let s = vld::string()
        .with_messages(|k| (k == "too_small").then(|| "short!".to_string()))
        .min(3);
    assert_eq!(msg(&s, json!("ab")), "short!");
    let n = vld::number()
        .with_messages(|k| (k == "too_big").then(|| "big!".to_string()))
        .max(1.0);
    assert_eq!(msg(&n, json!(2)), "big!");
    // A per-check message wins over the bulk closure.
    let s = vld::string()
        .min_msg(3, "own")
        .with_messages(|_| Some("bulk".into()));
    assert_eq!(msg(&s, json!("ab")), "own");
}

// 31. Path existence failures have their own codes.
#[test]
fn f31_path_codes() {
    let dir = crate::common::tempdir();
    let file = dir.path().join("f.txt");
    std::fs::write(&file, b"x").unwrap();
    let s = |p: &std::path::Path| json!(p.to_string_lossy());
    let code = |schema: &vld::primitives::ZPath, v| {
        crate::common::parse_err(schema, v).issues[0].code.clone()
    };
    assert_eq!(
        code(&vld::path().exists(), s(&dir.path().join("nope"))),
        IssueCode::PathNotFound
    );
    assert_eq!(code(&vld::path().file(), s(dir.path())), IssueCode::PathNotFile);
    assert_eq!(code(&vld::path().dir(), s(&file)), IssueCode::PathNotDir);
    #[cfg(feature = "file")]
    {
        let err = crate::common::parse_err(&vld::file(), s(dir.path()));
        assert_eq!(err.issues[0].code, IssueCode::PathNotFile);
    }
}

// 32. Container type errors carry the received value.
#[test]
fn f32_container_received() {
    let one = Some(json!(1));
    let rec = |e: VldError| e.issues[0].received.clone();
    assert_eq!(rec(vld::array(vld::string()).parse("1").unwrap_err()), one);
    assert_eq!(rec(vld::object().parse("1").unwrap_err()), one);
    assert_eq!(rec(vld::record(vld::string()).parse("1").unwrap_err()), one);
    assert_eq!(rec(vld::set(vld::string()).parse("1").unwrap_err()), one);
    assert_eq!(rec(vld::map(vld::string(), vld::string()).parse("1").unwrap_err()), one);
    assert_eq!(rec((vld::string(),).parse("1").unwrap_err()), one);
}

// 24. Lax predicates tightened.
#[test]
fn f24_strict_predicates() {
    let ok = |s: &ZString, v: &str| s.parse_value(&json!(v)).is_ok();
    let table: Vec<(ZString, &[&str], &[&str])> = vec![
        (vld::string().semver(), &["1.2.3", "v1.2.3"], &[" 1.2.3 ", "1.2.3 "]),
        (
            vld::string().color(),
            &["rgb(1,2,3)", "rgb(255, 0, 0)", "hsl(120, 50%, 50%)"],
            &["rgb(+1,+2,+3)", "hsl(+120, 50%, 50%)", "hsl(120, +50%, 50%)"],
        ),
        (
            vld::string().emoji(),
            &["😀", "👍🏽", "❤\u{FE0F}", "👨\u{200D}👩"],
            &["\u{FE0F}", "\u{200D}", "\u{FE0F}\u{200D}", ""],
        ),
        (
            vld::string().ulid(),
            &["01ARZ3NDEKTSV4RRFFQ69G5FAV", "7ZZZZZZZZZZZZZZZZZZZZZZZZZ"],
            &["8ZZZZZZZZZZZZZZZZZZZZZZZZZ", "ZZZZZZZZZZZZZZZZZZZZZZZZZZ"],
        ),
        (
            vld::string().hostname(),
            &["example.com", "a1.b2", "localhost", "123"],
            &["999.1.1.1", "1.2.3.4", "example.123"],
        ),
    ];
    for (s, good, bad) in &table {
        for v in *good {
            assert!(ok(s, v), "{v:?} rejected");
        }
        for v in *bad {
            assert!(!ok(s, v), "{v:?} accepted");
        }
    }
    let d = vld::duration();
    assert!(d.parse(r#""250ms""#).is_ok());
    assert!(d.parse(r#""+250ms""#).is_err());
    assert!(d.parse(r#""ms""#).is_err());
}

// 34. extend/merge keep the other schema's settings; when-targets are known;
// duplicate field() replaces.
#[test]
fn f34_object_merge() {
    let base = vld::object().field("a", vld::string());
    let other = vld::object()
        .field("b", vld::string())
        .strict()
        .min_keys(2)
        .when("b", "x", "c", vld::string().min(3));
    let m = base.extend(other);
    assert!(m.parse(r#"{"a": "1", "b": "y", "z": 1}"#).is_err()); // strict kept
    assert!(m.parse(r#"{"a": "1"}"#).is_err()); // min_keys kept (and b missing)
    // when-rule kept; its target is not "unrecognized" under strict
    assert!(m.parse(r#"{"a": "1", "b": "x", "c": "ab"}"#).is_err());
    assert!(m.parse(r#"{"a": "1", "b": "x", "c": "abc"}"#).is_ok());

    let o = vld::object()
        .field("a", vld::string())
        .field("a", vld::number());
    assert_eq!(o.keyof(), vec!["a".to_string()]);
    assert!(o.parse(r#"{"a": 1}"#).is_ok());
    #[cfg(feature = "openapi")]
    assert_eq!(
        vld::openapi::JsonSchema::json_schema(&o)["required"],
        json!(["a"])
    );
}

// 27. diff covers more keywords, stricter = breaking.
#[cfg(feature = "diff")]
#[test]
fn f27_diff_keywords() {
    use vld::diff::{diff_schemas, ChangeKind};
    let kind = |old: serde_json::Value, new: serde_json::Value| {
        let d = diff_schemas(&old, &new);
        assert_eq!(d.changes.len(), 1, "{d}");
        d.changes[0].kind
    };
    use ChangeKind::{Breaking as B, NonBreaking as N};
    let cases = [
        (json!({"const": 1}), json!({"const": 2}), B),
        (json!({}), json!({"const": 2}), B),
        (json!({"const": 1}), json!({}), N),
        (json!({"multipleOf": 2}), json!({"multipleOf": 4}), B),
        (json!({"multipleOf": 4}), json!({"multipleOf": 2}), N),
        (json!({"multipleOf": 2}), json!({"multipleOf": 3}), B),
        (json!({"prefixItems": [{}]}), json!({"prefixItems": [{}, {}]}), B),
        (json!({"prefixItems": [{}, {}]}), json!({"prefixItems": [{}]}), N),
        (
            json!({"prefixItems": [{"type": "string"}]}),
            json!({"prefixItems": [{"type": "number"}]}),
            B,
        ),
        (json!({}), json!({"uniqueItems": true}), B),
        (json!({"uniqueItems": true}), json!({}), N),
        (json!({"minProperties": 1}), json!({"minProperties": 2}), B),
        (json!({"maxProperties": 2}), json!({"maxProperties": 3}), N),
        (json!({}), json!({"propertyNames": {"pattern": "^a"}}), B),
        (
            json!({"propertyNames": {"pattern": "^a"}}),
            json!({"propertyNames": {}}),
            N,
        ),
        (json!({}), json!({"contains": {"const": 1}}), B),
        (json!({"minContains": 1}), json!({"minContains": 2}), B),
        (json!({"maxContains": 3}), json!({"maxContains": 2}), B),
        (
            json!({"formatMinimum": "2020-01-01"}),
            json!({"formatMinimum": "2021-01-01"}),
            B,
        ),
        (
            json!({"formatMaximum": "2020-01-01"}),
            json!({"formatMaximum": "2021-01-01"}),
            N,
        ),
        (json!({"x-minDecimal": "1.5"}), json!({"x-minDecimal": "2"}), B),
        (json!({"x-maxUniqueItems": 5}), json!({"x-maxUniqueItems": 6}), N),
        (json!({}), json!({"x-past": true}), B),
    ];
    for (old, new, want) in cases {
        assert_eq!(kind(old.clone(), new.clone()), want, "{old} -> {new}");
    }
}

// 37. impl_rules! reports serialization failures as serialize_error.
struct F37 {
    m: std::collections::HashMap<(i32, i32), i32>,
}

vld::impl_rules!(F37 {
    m => vld::any(),
});

#[test]
fn f37_impl_rules_serialize_error() {
    let v = F37 {
        m: [((1, 2), 3)].into_iter().collect(),
    };
    let err = v.validate().unwrap_err();
    assert_eq!(err.issues[0].code.key(), "serialize_error");
    assert_eq!(crate::common::paths(&err), [".m"]);
}

// 22. Discriminator values compare numerically (1 == 1.0).
#[test]
fn f22_discriminator_numeric_equality() {
    let s = vld::discriminated_union("v").variant(1, vld::object().field("v", vld::number()));
    assert!(s.parse(r#"{"v": 1.0}"#).is_ok());
    assert!(s.parse(r#"{"v": 1}"#).is_ok());
    assert!(s.parse(r#"{"v": 2}"#).is_err());
}

// 35. Literals of every integer type and f32.
#[test]
fn f35_literal_int_types() {
    assert_eq!(vld::literal(5u8).parse("5").unwrap(), 5u8);
    assert_eq!(vld::literal(5i32).parse("5.0").unwrap(), 5i32);
    assert_eq!(
        vld::literal(u64::MAX).parse("18446744073709551615").unwrap(),
        u64::MAX
    );
    assert!(vld::literal(5u16).parse("6").is_err());
    assert_eq!(vld::literal(-3isize).parse("-3").unwrap(), -3isize);
    assert_eq!(vld::literal(0.1f32).parse("0.1").unwrap(), 0.1f32);
}

// 13. string().iso_datetime() and the backends' datetime() share one grammar.
#[test]
fn f13_iso_datetime_grammar() {
    let s = vld::string().iso_datetime();
    for ok in [
        "2024-01-15T10:30:00Z",
        "2024-01-15t10:30:00z",
        "2024-01-15T10:30:00.5+02:00",
        "2024-01-15T10:30:00",
    ] {
        assert!(s.parse_value(&json!(ok)).is_ok(), "{ok}");
        #[cfg(feature = "chrono")]
        assert!(
            vld::chrono::datetime().parse_value(&json!(ok)).is_ok(),
            "{ok}"
        );
    }
    for bad in ["2024-01-15T10:30Z", "2024-01-15T10:30", "2024-01-15 10:30:00Z"] {
        assert!(s.parse_value(&json!(bad)).is_err(), "{bad}");
        #[cfg(feature = "chrono")]
        assert!(
            vld::chrono::datetime().parse_value(&json!(bad)).is_err(),
            "{bad}"
        );
    }
}

#[cfg(feature = "openapi")]
mod json_schema {
    use serde_json::json;
    use vld::openapi::JsonSchema;
    use vld::prelude::*;

    // 10. Schemas that accept a missing key are not `required`.
    #[test]
    fn f10_not_required() {
        let o = vld::object()
            .field("c", vld::string().catch("x".into()))
            .field("a", vld::any())
            .field("n", vld::null())
            .field("j", vld::json_value())
            .field("jo", vld::json_value().object())
            .field("s", vld::string());
        assert_eq!(o.json_schema()["required"], json!(["jo", "s"]));
        assert!(o.parse(r#"{"jo": {}, "s": "x"}"#).is_ok());
    }

    // 11. time-of-day schema: a pattern, not `format: time`.
    #[cfg(feature = "chrono")]
    #[test]
    fn f11_time_pattern() {
        let js = vld::chrono::time().json_schema();
        assert!(js.get("format").is_none());
        assert_eq!(js["pattern"], r"^\d{2}:\d{2}:\d{2}(\.\d{1,9})?$");
    }

    // 12. datetime() accepting naive input is not plain `date-time`.
    #[cfg(feature = "chrono")]
    #[test]
    fn f12_datetime_naive_schema() {
        let js = vld::chrono::datetime().json_schema();
        assert!(js.get("format").is_none());
        assert_eq!(js["anyOf"][0], json!({"format": "date-time"}));
        assert!(js["anyOf"][1]["pattern"].is_string());
        let js = vld::chrono::datetime().with_timezone_only().json_schema();
        assert_eq!(js["format"], "date-time");
    }

    // 14. decimal accepts numbers.
    #[cfg(feature = "decimal")]
    #[test]
    fn f14_decimal_type() {
        assert_eq!(
            vld::decimal().json_schema()["type"],
            json!(["string", "number"])
        );
    }

    // 15. Encoded bytes also accept arrays.
    #[test]
    fn f15_bytes_any_of() {
        let js = vld::bytes().base64().json_schema();
        assert_eq!(js["anyOf"][0]["type"], "string");
        assert_eq!(js["anyOf"][1]["type"], "array");
        assert!(vld::bytes().base64().parse("[1, 2]").is_ok());
        assert_eq!(vld::bytes().json_schema()["type"], "array");
    }

    // 16. ZFile describes its input.
    #[cfg(feature = "file")]
    #[test]
    fn f16_file_input_schema() {
        let js = vld::file().json_schema();
        assert_eq!(js["anyOf"][0], json!({"type": "string"}));
        assert_eq!(js["anyOf"][1]["required"], json!(["path"]));
    }

    // 17. require_key implies an object.
    #[test]
    fn f17_json_value_required_keys() {
        let js = vld::json_value().require_key("a").json_schema();
        assert_eq!(js["type"], "object");
        assert_eq!(js["required"], json!(["a"]));
    }

    // 18. multipleOf 0 is omitted.
    #[test]
    fn f18_multiple_of_zero() {
        assert!(vld::number()
            .multiple_of(0.0)
            .json_schema()
            .get("multipleOf")
            .is_none());
        assert_eq!(vld::number().multiple_of(2.0).json_schema()["multipleOf"], 2.0);
    }

    // 25. output_json passes through wrappers (RFC 3339 default).
    #[cfg(feature = "time")]
    #[test]
    fn f25_default_through_describe() {
        let dt = vld::time::datetime()
            .parse_value(&json!("2024-01-02T03:04:05Z"))
            .unwrap();
        let js = vld::time::datetime()
            .describe("when")
            .with_default(dt)
            .json_schema();
        assert_eq!(js["default"], "2024-01-02T03:04:05Z");
        let js = vld::time::datetime()
            .optional()
            .with_default(Some(dt))
            .json_schema();
        assert_eq!(js["default"], "2024-01-02T03:04:05Z");
    }

    // 41. past/future and set min_size hints.
    #[test]
    fn f41_generator_hints() {
        #[cfg(feature = "chrono")]
        {
            assert_eq!(vld::chrono::date().past().json_schema()["x-past"], true);
            assert_eq!(
                vld::chrono::datetime().future().json_schema()["x-future"],
                true
            );
        }
        let js = vld::set(vld::string()).min_size(2).json_schema();
        assert_eq!(js["x-minUniqueItems"], 2);
    }
}
