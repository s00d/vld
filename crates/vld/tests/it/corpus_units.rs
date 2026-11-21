//! Corpus: values with units — ISO 8601 durations (accepted forms, calendar units, near-misses),
//! `money()` against ISO 4217 minor units, decimal scale of JSON numbers, and `range()` over
//! numbers.

use crate::common::*;
use serde_json::json;
use vld::prelude::*;

fn root(code: &str) -> Vec<(String, String)> {
    vec![(String::new(), code.to_string())]
}

/// Given `duration()`
/// When ISO 8601 durations of every shape, years / months, and near-misses are parsed
/// Then weeks, days and time parts (one fraction, on the last number, `.` or `,`) give the exact
/// duration; years and months are `invalid_format` (`duration_calendar_unit`) even when combined;
/// near-misses (empty parts, wrong order, repeated units, `T` without a time, weeks mixed with
/// days, signs, spaces, lower case, more than 9 fraction digits, overflow) are `invalid_type`.
#[test]
fn iso_durations() {
    use std::time::Duration;
    let d = vld::duration();
    let ms = Duration::from_millis;
    for (text, want) in [
        ("PT15S", ms(15_000)),
        ("PT1.5S", ms(1_500)),
        ("PT0S", ms(0)),
        ("PT1M", ms(60_000)),
        ("PT1H", ms(3_600_000)),
        ("PT1H30M", ms(5_400_000)),
        ("PT1.5H", ms(5_400_000)),
        ("PT0,5M", ms(30_000)),
        ("P1D", ms(86_400_000)),
        ("P3DT4H30M", ms(275_400_000)),
        ("P1DT1S", ms(86_401_000)),
        ("P0.5D", ms(43_200_000)),
        ("P2W", ms(1_209_600_000)),
        ("P0.5W", ms(302_400_000)),
        ("PT0.000000001S", Duration::from_nanos(1)),
        ("PT1.123456789S", Duration::new(1, 123_456_789)),
        ("PT0.1H", ms(360_000)),
        ("PT100000H", Duration::from_secs(360_000_000)),
    ] {
        assert_eq!(d.parse_value(&json!(text)).ok(), Some(want), "{text}");
    }
    for calendar in ["P1Y", "P2M", "P1Y2M3D", "P1MT1H", "P0Y", "P1.5Y"] {
        let err = d.parse_value(&json!(calendar)).unwrap_err();
        assert_eq!(issues(&err), root("invalid_format"), "{calendar}");
        assert_eq!(
            err.issues[0].code,
            IssueCode::InvalidFormat {
                format: vld::error::FormatKind::DurationCalendarUnit
            }
        );
    }
    for bad in [
        "P",
        "PT",
        "P1DT",
        "PT1H2H",
        "PT1S1M",
        "P1W1D",
        "P1WT1H",
        "PT1D",
        "P1H",
        "P1.5DT1H",
        "PT1.5H30M",
        "P-1D",
        "P+1D",
        "-PT1S",
        "PT 1S",
        "pt1s",
        "PT1s",
        "PT1.S",
        "PT.5S",
        "PT1.1234567890S",
        "PT1..5S",
        "P1DT1H1M1S1",
        "PT18446744073709551616S",
        "P99999999999999999W",
        "PT１S",
    ] {
        assert_eq!(
            issues(&d.parse_value(&json!(bad)).unwrap_err()),
            root("invalid_type"),
            "{bad}"
        );
    }
    // Bounds apply to ISO input too.
    let short = vld::duration().max_secs(3600);
    assert!(short.parse_value(&json!("PT1H")).is_ok());
    assert_eq!(
        issues(&short.parse_value(&json!("PT1H0.001S")).unwrap_err()),
        root("too_big")
    );
}

/// Given `money()`, bare, with a stricter amount schema and with an allow list
/// When amounts in currencies with 0, 2, 3 and 4 minor units, unknown / lower-case codes,
/// missing fields and wrong types are parsed
/// Then the amount keeps its written scale and may have at most the currency's minor units
/// (`too_big`, origin `scale`, at `.amount`); unknown codes are `invalid_string` at
/// `.currency`, codes outside the allow list `invalid_enum_value`; both fields' issues are
/// reported; the output serializes back with the amount as a string.
#[cfg(feature = "decimal")]
#[test]
fn money_against_minor_units() {
    use vld::error::SizeOrigin;
    let m = vld::money();
    for (amount, currency) in [
        (json!("12.50"), "USD"),
        (json!("12.5"), "USD"),
        (json!("12"), "USD"),
        (json!(12.5), "EUR"),
        (json!("1000"), "JPY"),
        (json!(1000), "KRW"),
        (json!("1.250"), "BHD"),
        (json!("0.001"), "KWD"),
        (json!("1.2345"), "CLF"),
        (json!("-3.99"), "GBP"),
        (json!("1e2"), "JPY"),
    ] {
        let v = json!({"amount": amount, "currency": currency});
        let out = m.parse_value(&v).unwrap_or_else(|e| panic!("{v}: {e}"));
        assert_eq!(out.currency, currency);
    }
    let out = m
        .parse_value(&json!({"amount": "12.50", "currency": "USD"}))
        .unwrap();
    assert_eq!(out.amount.to_string(), "12.50");
    assert_eq!(
        serde_json::to_value(&out).unwrap(),
        json!({"amount": "12.50", "currency": "USD"})
    );

    for (v, want) in [
        (json!({"amount": "12.505", "currency": "USD"}), 2),
        (json!({"amount": "12.500", "currency": "USD"}), 2),
        (json!({"amount": "1.5", "currency": "JPY"}), 0),
        (json!({"amount": 1.5, "currency": "JPY"}), 0),
        (json!({"amount": "1.2345", "currency": "BHD"}), 3),
        (json!({"amount": "1.23456", "currency": "CLF"}), 4),
    ] {
        let err = m.parse_value(&v).unwrap_err();
        assert_eq!(issues(&err), [(".amount".into(), "too_big".into())], "{v}");
        assert_eq!(
            err.issues[0].code,
            IssueCode::TooBig {
                origin: SizeOrigin::Scale,
                maximum: json!(want),
                inclusive: true
            }
        );
    }
    let at = |p: &str, c: &str| (p.to_string(), c.to_string());
    for (v, want) in [
        (
            json!({"amount": "1", "currency": "usd"}),
            vec![at(".currency", "invalid_string")],
        ),
        (
            json!({"amount": "1", "currency": "XAU"}),
            vec![at(".currency", "invalid_string")],
        ),
        (
            json!({"amount": "1", "currency": "ZZZ"}),
            vec![at(".currency", "invalid_string")],
        ),
        (
            json!({"amount": "1", "currency": 840}),
            vec![at(".currency", "invalid_type")],
        ),
        (
            json!({"amount": "x", "currency": "ZZZ"}),
            vec![
                at(".amount", "invalid_format"),
                at(".currency", "invalid_string"),
            ],
        ),
        // Without a valid currency only the amount's own rules apply.
        (
            json!({"amount": "1.23456", "currency": "ZZZ"}),
            vec![at(".currency", "invalid_string")],
        ),
        (
            json!({"currency": "USD"}),
            vec![at(".amount", "missing_field")],
        ),
        (
            json!({}),
            vec![
                at(".amount", "missing_field"),
                at(".currency", "missing_field"),
            ],
        ),
        (
            json!({"amount": null, "currency": "USD"}),
            vec![at(".amount", "invalid_type")],
        ),
        (json!("12 USD"), vec![at("", "invalid_type")]),
    ] {
        assert_eq!(issues(&m.parse_value(&v).unwrap_err()), want, "{v}");
    }

    let pos = vld::money().amount(vld::decimal().positive().max_scale(1));
    assert_eq!(
        issues(
            &pos.parse_value(&json!({"amount": "0", "currency": "USD"}))
                .unwrap_err()
        ),
        [at(".amount", "too_small")]
    );
    // The stricter of the two scale rules wins.
    assert_eq!(
        issues(
            &pos.parse_value(&json!({"amount": "1.25", "currency": "USD"}))
                .unwrap_err()
        ),
        [at(".amount", "too_big")]
    );
    let eur = vld::money().currencies(&["EUR", "USD"]);
    assert!(
        eur.parse_value(&json!({"amount": "1", "currency": "USD"}))
            .is_ok()
    );
    let err = eur
        .parse_value(&json!({"amount": "1", "currency": "GBP"}))
        .unwrap_err();
    assert_eq!(issues(&err), [at(".currency", "invalid_enum_value")]);
    assert!(
        std::panic::catch_unwind(|| drop(vld::money().currencies(&["EURO"]))).is_err(),
        "an unknown code in the allow list panics"
    );

    #[cfg(feature = "openapi")]
    {
        use vld::openapi::JsonSchema;
        assert_eq!(
            vld::money().json_schema(),
            json!({
                "type": "object",
                "properties": {
                    "amount": {"type": ["string", "number"], "format": "decimal"},
                    "currency": {"type": "string", "format": "currency-code"},
                },
                "required": ["amount", "currency"],
                "x-money": true,
            })
        );
        assert_eq!(
            eur.json_schema()["properties"]["currency"],
            json!({"type": "string", "enum": ["EUR", "USD"]})
        );
        use vld::schema::VldType;
        assert_eq!(
            vld::primitives::Money::vld_schema().json_schema(),
            vld::money().json_schema()
        );
    }
}

/// Given `decimal()` with scale and precision checks
/// When JSON numbers (not strings) and scientific notation arrive
/// Then a JSON number has the scale of its shortest form (`1.10` reads as `1.1`), scientific
/// notation the scale of its value (`1e-7` → 7, `1.5e2` → 0), and precision ignores leading
/// zeros.
#[cfg(feature = "decimal")]
#[test]
fn decimal_scale_of_numbers_and_exponents() {
    let s2 = vld::decimal().max_scale(2);
    for ok in [
        json!(1.1),
        json!(1.10),
        json!(100),
        json!(0.01),
        json!("1.5e2"),
    ] {
        assert!(s2.parse_value(&ok).is_ok(), "{ok}");
    }
    for bad in [json!(0.001), json!("1e-7"), json!("1.230")] {
        assert_eq!(
            issues(&s2.parse_value(&bad).unwrap_err()),
            root("too_big"),
            "{bad}"
        );
    }
    let exact = vld::decimal().scale(2);
    assert!(exact.parse_value(&json!("1.50")).is_ok());
    assert_eq!(
        issues(&exact.parse_value(&json!(1.50)).unwrap_err()),
        root("invalid_length")
    );
    let p3 = vld::decimal().max_precision(3);
    for ok in ["0.012", "-999", "1.20", "0.00"] {
        assert!(p3.parse_value(&json!(ok)).is_ok(), "{ok}");
    }
    for bad in ["1000", "0.0001", "1.230"] {
        assert_eq!(
            issues(&p3.parse_value(&json!(bad)).unwrap_err()),
            root("too_big"),
            "{bad}"
        );
    }
    assert!(std::panic::catch_unwind(|| drop(vld::decimal().max_precision(0))).is_err());
    #[cfg(feature = "openapi")]
    {
        use vld::schema::VldType;
        assert!(
            rust_decimal::Decimal::vld_schema()
                .parse_value(&json!("1.5"))
                .is_ok()
        );
    }
}

/// Given `range()` over integers, floats and decimals, with renamed keys
/// When ordered, equal, reversed and malformed pairs are parsed
/// Then ordered and equal ends give `start..=end`; a reversed pair is one `range_order` at the
/// end field naming both fields; item issues are reported per field (both missing, both wrong);
/// a non-object is `invalid_type`; the JSON Schema is an object with both fields required and
/// `x-rangeOrder`.
#[test]
fn numeric_ranges() {
    let r = vld::range(vld::i32());
    assert_eq!(
        r.parse_value(&json!({"start": -5, "end": 5})).unwrap(),
        -5..=5
    );
    assert_eq!(
        r.parse_value(&json!({"start": 3, "end": 3})).unwrap(),
        3..=3
    );
    let err = r.parse_value(&json!({"start": 4, "end": 3})).unwrap_err();
    assert_eq!(issues(&err), [(".end".into(), "range_order".into())]);
    assert_eq!(err.issues[0].received, Some(json!(3)));
    assert_eq!(
        issues(&r.parse_value(&json!({})).unwrap_err()),
        [
            (".start".into(), "missing_field".into()),
            (".end".into(), "missing_field".into())
        ]
    );
    assert_eq!(
        issues(
            &r.parse_value(&json!({"start": "a", "end": 1.5}))
                .unwrap_err()
        ),
        [
            (".start".into(), "invalid_type".into()),
            (".end".into(), "not_int".into())
        ]
    );
    assert_eq!(
        issues(&r.parse_value(&json!([1, 2])).unwrap_err()),
        root("invalid_type")
    );

    let f = vld::range(vld::number()).keys("low", "high");
    assert_eq!(
        f.parse_value(&json!({"low": 0.5, "high": 0.75})).unwrap(),
        0.5..=0.75
    );
    let err = f
        .parse_value(&json!({"low": 0.75, "high": 0.5}))
        .unwrap_err();
    assert_eq!(issues(&err), [(".high".into(), "range_order".into())]);
    assert_eq!(
        err.issues[0].code,
        IssueCode::RangeOrder {
            start: "low".into(),
            end: "high".into()
        }
    );

    #[cfg(feature = "decimal")]
    {
        let d = vld::range(vld::decimal());
        // Compared as values, not text: "10" > "9.5".
        assert!(d.parse_value(&json!({"start": "9.5", "end": "10"})).is_ok());
        assert_eq!(
            issues(
                &d.parse_value(&json!({"start": "10.0", "end": "9.99"}))
                    .unwrap_err()
            ),
            [(".end".into(), "range_order".into())]
        );
    }

    #[cfg(feature = "openapi")]
    {
        use vld::openapi::JsonSchema;
        let item = vld::i32().json_schema();
        assert_eq!(
            r.json_schema(),
            json!({
                "type": "object",
                "properties": {"start": item, "end": item},
                "required": ["start", "end"],
                "x-rangeOrder": ["start", "end"],
            })
        );
        assert_eq!(f.json_schema()["x-rangeOrder"], json!(["low", "high"]));
    }
}
