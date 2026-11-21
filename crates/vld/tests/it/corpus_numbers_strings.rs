//! Corpus: number and string cases the models do not generate.
//!
//! Full issue payloads (bound, inclusiveness, origin, divisor, string
//! validation, received type), fractional and `f32`-precision bounds, float
//! `multiple_of`, integers beyond `f64` precision, `usize` / `isize`,
//! `i16::MIN`, non-ASCII digits and non-finite text under `coerce`, string
//! formats mixed with length checks; text coercion (`coerce_text`) of
//! number-, boolean- and null-like text and its round trip through
//! `parse_coerced`. Integer bounds, sign checks, type ranges,
//! clamp, coerce and custom messages are `model_numbers`; transforms, length,
//! substring and character-class checks are `model_strings`.

use crate::common::*;

use serde_json::{Value, json};
use vld::error::{IssueCode, SizeOrigin, StringValidation};
use vld::prelude::*;

/// The output, or every issue code (with its payload) in order.
type Want = Result<Value, Vec<IssueCode>>;

fn run(rows: Vec<(Erased, Value, Want)>) {
    for (i, (schema, input, want)) in rows.into_iter().enumerate() {
        let got = schema
            .parse_value(&input)
            .map_err(|e| e.issues.into_iter().map(|i| i.code).collect::<Vec<_>>());
        assert_eq!(got, want, "row {i}: input {input}");
    }
}

fn e<S: vld::schema::DynSchema + Send + Sync + 'static>(s: S) -> Erased {
    Erased::new(s)
}

fn ok(v: Value) -> Want {
    Ok(v)
}

fn small(minimum: Value, inclusive: bool) -> IssueCode {
    IssueCode::TooSmall {
        origin: SizeOrigin::Number,
        minimum,
        inclusive,
    }
}

fn big(maximum: Value, inclusive: bool) -> IssueCode {
    IssueCode::TooBig {
        origin: SizeOrigin::Number,
        maximum,
        inclusive,
    }
}

fn not_a(expected: &str, received: &str) -> IssueCode {
    IssueCode::InvalidType {
        expected: expected.into(),
        received: received.into(),
    }
}

/// Given `number()` / `f32()` schemas with fractional bounds, float
/// `multiple_of`, `safe` and `coerce`
/// When values at and around each bound are parsed
/// Then the output is the number, or the issue carries the bound, whether it
/// is inclusive, and the divisor; non-finite and non-ASCII-digit text never
/// coerces; the JSON Schema keeps the exclusive bound of two equal ones.
#[test]
fn float_bounds_and_payloads() {
    let mut rows = vec![
        (e(vld::number()), json!(42.5), ok(json!(42.5))),
        (e(vld::number()), json!(-3), ok(json!(-3.0))),
        (
            e(vld::number()),
            json!("1"),
            Err(vec![not_a("number", "string")]),
        ),
        (
            e(vld::number().min(0.0)),
            json!(-0.0001),
            Err(vec![small(json!(0.0), true)]),
        ),
        (e(vld::number().gte(0.0)), json!(0.0), ok(json!(0.0))),
        (
            e(vld::number().gte(0.0)),
            json!(-1),
            Err(vec![small(json!(0.0), true)]),
        ),
        (
            e(vld::number().gt(0.0)),
            json!(0.0),
            Err(vec![small(json!(0.0), false)]),
        ),
        (e(vld::number().gt(0.0)), json!(1e-9), ok(json!(1e-9))),
        (
            e(vld::number().max(1.0)),
            json!(1.0001),
            Err(vec![big(json!(1.0), true)]),
        ),
        (e(vld::number().lte(1.0)), json!(1.0), ok(json!(1.0))),
        (
            e(vld::number().lt(1.0)),
            json!(1.0),
            Err(vec![big(json!(1.0), false)]),
        ),
        (e(vld::number().lt(1.0)), json!(0.999), ok(json!(0.999))),
        (e(vld::number().positive()), json!(0.1), ok(json!(0.1))),
        (e(vld::number().negative()), json!(-0.1), ok(json!(-0.1))),
        (
            e(vld::number().gt(5.0)),
            json!(5),
            Err(vec![small(json!(5.0), false)]),
        ),
        (
            e(vld::number().lte(5.0)),
            json!(6),
            Err(vec![big(json!(5.0), true)]),
        ),
        // multiple_of with float tolerance; a zero divisor never matches.
        (
            e(vld::number().multiple_of(0.1)),
            json!(0.3),
            ok(json!(0.3)),
        ),
        (
            e(vld::number().multiple_of(0.1)),
            json!(-0.3),
            ok(json!(-0.3)),
        ),
        (
            e(vld::number().multiple_of(0.1)),
            json!(0.0),
            ok(json!(0.0)),
        ),
        (
            e(vld::number().multiple_of(0.1)),
            json!(1.1),
            ok(json!(1.1)),
        ),
        (
            e(vld::number().multiple_of(0.1)),
            json!(0.35),
            Err(vec![IssueCode::NotMultipleOf {
                divisor: json!(0.1),
            }]),
        ),
        // A whole input and a fractional divisor; the tolerance grows with the quotient.
        (e(vld::number().multiple_of(0.1)), json!(1), ok(json!(1.0))),
        (
            e(vld::number().multiple_of(0.1)),
            json!(10000000.1),
            ok(json!(10000000.1)),
        ),
        // `one_of` lists the allowed values as text.
        (
            e(vld::number().one_of(&[1.0, 2.5])),
            json!(3),
            Err(vec![IssueCode::InvalidEnumValue {
                options: vec!["1".into(), "2.5".into()],
            }]),
        ),
        // An infinite divisor never matches.
        (
            e(vld::number().multiple_of(f64::INFINITY)),
            json!(5),
            Err(vec![IssueCode::NotMultipleOf {
                divisor: Value::Null,
            }]),
        ),
        (
            e(vld::number().multiple_of(0.0)),
            json!(5),
            Err(vec![IssueCode::NotMultipleOf {
                divisor: json!(0.0),
            }]),
        ),
        (
            e(vld::number().safe()),
            json!(-9007199254740991.0),
            ok(json!(-9007199254740991.0)),
        ),
        (
            e(vld::number().safe()),
            json!(-9007199254740992_i64),
            Err(vec![IssueCode::NotSafeInteger]),
        ),
        (e(vld::number().coerce()), json!("2.5"), ok(json!(2.5))),
        (e(vld::number().coerce()), json!(3), ok(json!(3.0))),
        // f32: range, exact extremes, bounds compared at f32 precision.
        (e(vld::f32()), json!(1.5), ok(json!(1.5f32))),
        (
            e(vld::f32()),
            json!(1e39),
            Err(vec![big(json!(f32::MAX as f64), true)]),
        ),
        (
            e(vld::f32()),
            json!(-1e39),
            Err(vec![small(json!(f32::MIN as f64), true)]),
        ),
        (e(vld::f32()), json!(f32::MAX as f64), ok(json!(f32::MAX))),
        (e(vld::f32()), json!(3.4028235e38), ok(json!(f32::MAX))),
        (e(vld::f32()), json!(-3.4028235e38), ok(json!(f32::MIN))),
        (
            e(vld::f32().min(0.1).max(0.1)),
            json!(0.1),
            ok(json!(0.1f32)),
        ),
        (
            e(vld::f32().multiple_of(0.1)),
            json!(0.3),
            ok(json!(0.3f32)),
        ),
        (
            e(vld::f32().multiple_of(0.5)),
            json!(2.5),
            ok(json!(2.5f32)),
        ),
        (
            e(vld::f32().gte(1.0).lte(2.0)),
            json!(1.5),
            ok(json!(1.5f32)),
        ),
        (e(vld::f32().coerce()), json!("2.5"), ok(json!(2.5f32))),
    ];
    for bad in ["NaN", "inf", "-infinity", "1e309", "abc", "", "١"] {
        rows.push((
            e(vld::number().coerce()),
            json!(bad),
            Err(vec![not_a("number", "string")]),
        ));
    }
    run(rows);

    // The JSON Schema keeps the strictest bound; on a tie the exclusive one, in either order.
    #[cfg(feature = "openapi")]
    {
        use vld::openapi::JsonSchema;
        let want = json!({"type": "number", "exclusiveMinimum": 5.0});
        assert_eq!(vld::number().gt(5.0).min(5.0).json_schema(), want);
        assert_eq!(vld::number().min(5.0).gt(5.0).json_schema(), want);
    }
}

/// Given integer schemas (`number().int()` and the sized types)
/// When values beyond `f64` precision, at type extremes the model does not
/// pick (`i16::MIN`, `usize`, `isize`, `u64` bounds near `u64::MAX`), floats
/// in exponent form and coerced text are parsed
/// Then integers stay exact, a user bound tighter than the type range is the
/// one reported, and the type range is reported otherwise.
#[test]
fn integer_extremes_and_payloads() {
    let mut rows = vec![
        (
            e(vld::number().int()),
            json!(9007199254740993_i64),
            ok(json!(9007199254740993_i64)),
        ),
        (
            e(vld::number().int()),
            json!(42.5),
            Err(vec![IssueCode::NotInt]),
        ),
        (
            e(vld::number().int().max(9007199254740992)),
            json!(9007199254740993_i64),
            Err(vec![big(json!(9007199254740992_i64), true)]),
        ),
        (
            e(vld::number().int().coerce()),
            json!("9007199254740993"),
            ok(json!(9007199254740993_i64)),
        ),
        (
            e(vld::number().int().coerce()),
            json!("4.5"),
            Err(vec![IssueCode::NotInt]),
        ),
        (e(vld::u8()), json!(256), Err(vec![big(json!(255), true)])),
        (e(vld::u8()), json!(-1), Err(vec![small(json!(0), true)])),
        (e(vld::u8()), json!(1e3), Err(vec![big(json!(255), true)])),
        (
            e(vld::u8().max(150)),
            json!(300),
            Err(vec![big(json!(150), true)]),
        ),
        (
            e(vld::u8().min(10)),
            json!(-5),
            Err(vec![small(json!(10), true)]),
        ),
        (e(vld::i16()), json!(-32768), ok(json!(i16::MIN))),
        (e(vld::i32()), json!(2.0), ok(json!(2))),
        (e(vld::usize()), json!(7), ok(json!(7))),
        (e(vld::isize()), json!(-7), ok(json!(-7))),
        (e(vld::usize()), json!(-7), Err(vec![small(json!(0), true)])),
        (e(vld::u64()), json!(-1), Err(vec![small(json!(0), true)])),
        (
            e(vld::u64().min(u64::MAX - 1)),
            json!(u64::MAX),
            ok(json!(u64::MAX)),
        ),
        (
            e(vld::u64().min(u64::MAX - 1)),
            json!(u64::MAX - 2),
            Err(vec![small(json!(u64::MAX - 1), true)]),
        ),
        (
            e(vld::u32().multiple_of(5)),
            json!(26),
            Err(vec![IssueCode::NotMultipleOf { divisor: json!(5) }]),
        ),
        (e(vld::u16().coerce()), json!("443"), ok(json!(443))),
        (
            e(vld::u64().coerce()),
            json!("18446744073709551615"),
            ok(json!(u64::MAX)),
        ),
    ];
    for bad in ["٤٢", "abc", ""] {
        rows.push((
            e(vld::number().int().coerce()),
            json!(bad),
            Err(vec![not_a("number", "string")]),
        ));
    }
    run(rows);
    // Beyond u64 serde_json reads a float: still out of range.
    assert!(vld::u64().parse("18446744073709551616").is_err());
    // `number().int()` is `ZInteger<i64>` under its old name.
    let _: ZInt = vld::number().int();
    let _: ZInteger<i64> = vld::number().int().min(1);
}

/// Given `f32()` bounds and `multiple_of` with a fractional value
/// When a value fails one of them
/// Then the default message prints the bound in its short `f32` form (`0.1`,
/// not `0.10000000149011612`).
#[test]
fn f32_bounds_print_their_short_form() {
    let err = vld::f32()
        .multiple_of(0.1)
        .parse_value(&json!(0.35))
        .unwrap_err();
    assert_eq!(err.issues[0].message, "Number must be a multiple of 0.1");
    let err = vld::f32().min(0.1).parse_value(&json!(0.05)).unwrap_err();
    assert_eq!(err.issues[0].message, "Number must be at least 0.1");
}

fn str_size(small: bool, n: usize) -> IssueCode {
    if small {
        IssueCode::TooSmall {
            origin: SizeOrigin::String,
            minimum: json!(n),
            inclusive: true,
        }
    } else {
        IssueCode::TooBig {
            origin: SizeOrigin::String,
            maximum: json!(n),
            inclusive: true,
        }
    }
}

fn invalid(validation: StringValidation) -> IssueCode {
    IssueCode::InvalidString { validation }
}

/// Given `string()` schemas with length, substring and format checks
/// When text, non-strings and coerced scalars are parsed
/// Then every failing check is reported in order with its payload (origin,
/// bound, prefix / suffix / substring, format), and a non-string is an
/// `invalid_type` that echoes the received value.
#[test]
fn string_payloads() {
    for (bad, kind) in [
        (json!(42), "number"),
        (json!(null), "null"),
        (json!(true), "boolean"),
        (json!([]), "array"),
        (json!({}), "object"),
    ] {
        let err = vld::string().parse_value(&bad).unwrap_err();
        assert_eq!(err.issues[0].code, not_a("string", kind));
        assert_eq!(err.issues[0].received, Some(bad));
    }
    let affixes = || {
        vld::string()
            .starts_with("foo")
            .ends_with("bar")
            .contains("oba")
    };
    #[allow(unused_mut)]
    let mut rows = vec![
        (e(vld::string()), json!(""), ok(json!(""))),
        (
            e(vld::string().min(2).max(3)),
            json!("日本語"),
            ok(json!("日本語")),
        ),
        (
            e(vld::string().min(2).max(3)),
            json!("a"),
            Err(vec![str_size(true, 2)]),
        ),
        (
            e(vld::string().min(2).max(3)),
            json!("abcd"),
            Err(vec![str_size(false, 3)]),
        ),
        (e(vld::string().len(3)), json!("été"), ok(json!("été"))),
        (
            e(vld::string().len(3)),
            json!("ab"),
            Err(vec![IssueCode::InvalidLength {
                origin: SizeOrigin::String,
                expected: 3,
            }]),
        ),
        (e(vld::string().non_empty()), json!(" "), ok(json!(" "))),
        (
            e(vld::string().non_empty()),
            json!(""),
            Err(vec![str_size(true, 1)]),
        ),
        (e(affixes()), json!("foobar"), ok(json!("foobar"))),
        (
            e(affixes()),
            json!("xyz"),
            Err(vec![
                invalid(StringValidation::StartsWith {
                    prefix: "foo".into(),
                }),
                invalid(StringValidation::EndsWith {
                    suffix: "bar".into(),
                }),
                invalid(StringValidation::Contains {
                    substring: "oba".into(),
                }),
            ]),
        ),
        (
            e(vld::string().min(5).email().url()),
            json!("ab"),
            Err(vec![
                str_size(true, 5),
                invalid(StringValidation::Email),
                invalid(StringValidation::Url),
            ]),
        ),
        (e(vld::string().coerce()), json!(1.5), ok(json!("1.5"))),
        (
            e(vld::string().coerce()),
            json!({"a": 1}),
            Err(vec![not_a("string", "object")]),
        ),
    ];
    #[cfg(feature = "regex")]
    {
        rows.push((
            e(vld::string().pattern(r"^[a-z]+\d$")),
            json!("abc1"),
            ok(json!("abc1")),
        ));
        for bad in ["abc", "ABC1", "", "é1"] {
            rows.push((
                e(vld::string().pattern(r"^[a-z]+\d$")),
                json!(bad),
                Err(vec![invalid(StringValidation::Regex)]),
            ));
        }
    }
    run(rows);
}

/// `coerce_text`: only text that prints back to itself becomes a number /
/// boolean; through `parse_coerced` every value follows its field's schema.
#[test]
fn text_coercion() {
    use vld::schema::{coerce_text, parse_coerced};
    let rows: &[(&str, Value)] = &[
        ("true", json!(true)),
        ("false", json!(false)),
        ("0", json!(0)),
        ("42", json!(42)),
        ("-7", json!(-7)),
        ("9223372036854775807", json!(i64::MAX)),
        ("18446744073709551615", json!(u64::MAX)),
        ("2.75", json!(2.75)),
        ("-0.5", json!(-0.5)),
        // Everything below stays text.
        ("18446744073709551616", json!("18446744073709551616")),
        ("", json!("")),
        ("null", json!("null")),
        ("TRUE", json!("TRUE")),
        ("True", json!("True")),
        ("01234", json!("01234")),
        ("+1555", json!("+1555")),
        ("-0", json!("-0")),
        ("1e3", json!("1e3")),
        ("1.50", json!("1.50")),
        ("1.", json!("1.")),
        (".5", json!(".5")),
        ("0.1000000000000000001", json!("0.1000000000000000001")),
        (" 1", json!(" 1")),
        ("NaN", json!("NaN")),
        ("inf", json!("inf")),
        ("١٢", json!("١٢")),
    ];
    for (raw, want) in rows {
        assert_eq!(&coerce_text(raw), want, "{raw:?}");
    }

    vld::schema! {
        #[derive(Debug)]
        struct Env {
            name: String => vld::string(),
            port: i64 => vld::number().int().min(1),
            ratio: f64 => vld::number(),
            debug: bool => vld::boolean(),
            zip: String => vld::string(),
            tags: Vec<String> => vld::array(vld::string()),
        }
    }
    let text = [
        ("name", "007"),
        ("port", "8080"),
        ("ratio", "0.25"),
        ("debug", "false"),
        ("zip", "12345"),
        ("tags", "true"),
    ];
    let value = Value::Object(
        text.iter()
            .map(|(k, v)| (k.to_string(), coerce_text(v)))
            .collect(),
    );
    let env: Env = parse_coerced(&value).unwrap();
    assert_eq!(
        (env.name.as_str(), env.port, env.ratio, env.debug),
        ("007", 8080, 0.25, false)
    );
    assert_eq!(
        (env.zip.as_str(), env.tags),
        ("12345", vec!["true".to_string()])
    );

    let value = json!({"name": coerce_text("x"), "port": coerce_text("0"), "ratio": coerce_text("1.0"),
        "debug": coerce_text("yes"), "zip": coerce_text("1"), "tags": []});
    let err = parse_coerced::<Env>(&value).unwrap_err();
    let got: Vec<_> = err
        .issues
        .iter()
        .map(|i| {
            (
                i.path.iter().map(ToString::to_string).collect::<String>(),
                i.code.key(),
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            (".port".to_string(), "too_small"),
            (".ratio".into(), "invalid_type"),
            (".debug".into(), "invalid_type")
        ]
    );
}
