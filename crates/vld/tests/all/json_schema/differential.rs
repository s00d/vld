//! Differential test: for each schema and sample, vld's verdict must equal
//! the `jsonschema` crate's verdict (draft 2020-12, formats asserted) on the
//! JSON Schema vld emits for that schema.
//!
//! Known, accepted divergences are listed in [`KNOWN_DIVERGENCES`] with the
//! reason; a listed pair that stops diverging fails the test too, so the list
//! stays exact.

use serde_json::{Value, json};
use vld::openapi::JsonSchema;
use vld::prelude::*;

struct Case {
    name: &'static str,
    schema: Value,
    vld_ok: Box<dyn Fn(&Value) -> bool>,
    samples: Vec<Value>,
}

fn case<S>(name: &'static str, schema: S, samples: Vec<Value>) -> Case
where
    S: VldSchema + JsonSchema + 'static,
{
    Case {
        name,
        schema: schema.json_schema(),
        vld_ok: Box::new(move |v| schema.parse_value(v).is_ok()),
        samples,
    }
}

/// `(case, sample, vld accepts?, reason)`.
const KNOWN_DIVERGENCES: &[(&str, &str, bool, &str)] = &[
    (
        "object_strict",
        r#"{"age":null,"name":"a"}"#,
        true,
        "optional() accepts `null` (serde writes `None` as `null`) but its schema has no `null` type (TypeScript `x?: T`)",
    ),
    (
        "iso_datetime",
        r#""2024-01-15T10:30:00""#,
        true,
        "vld accepts naive datetimes; RFC 3339 `date-time` requires an offset",
    ),
    (
        "iso_time",
        r#""10:30:00""#,
        true,
        "vld `time` has no offset; RFC 3339 `time` requires one",
    ),
    (
        "url",
        r#""ftp://example.com""#,
        false,
        "vld url() is http(s) only but emits the generic `uri` format",
    ),
    (
        "url",
        r#""mailto:a@b.co""#,
        false,
        "vld url() is http(s) only but emits the generic `uri` format",
    ),
    (
        "bytes_base64",
        r#""AQ==""#,
        false,
        "a decoded byte count maps to an encoded length; padding makes shorter payloads the same length",
    ),
    (
        "set_max_unique",
        "[1, 2, 3]",
        false,
        "set size bounds count unique items: emitted as x-maxUniqueItems (not enforced)",
    ),
];

fn cases() -> Vec<Case> {
    vec![
        case(
            "string_len",
            vld::string().min(2).max(3),
            vec![
                json!("ab"),
                json!("éé"),
                json!("a"),
                json!("abcd"),
                json!(1),
            ],
        ),
        case(
            "email",
            vld::string().email(),
            vec![json!("a@b.co"), json!("nope"), json!("@x.com")],
        ),
        case(
            "uuid",
            vld::string().uuid(),
            vec![
                json!("550e8400-e29b-41d4-a716-446655440000"),
                json!("550e8400"),
            ],
        ),
        case(
            "ipv4",
            vld::string().ipv4(),
            vec![json!("1.2.3.4"), json!("256.1.1.1"), json!("01.2.3.4")],
        ),
        case(
            "ipv6",
            vld::string().ipv6(),
            vec![
                json!("::1"),
                json!("fe80::1"),
                json!("1.2.3.4"),
                json!(":::"),
            ],
        ),
        case(
            "iso_date",
            vld::string().iso_date(),
            vec![
                json!("2024-02-29"),
                json!("2023-02-29"),
                json!("2024-1-1"),
                json!("2024-02-31"),
            ],
        ),
        case(
            "iso_datetime",
            vld::string().iso_datetime(),
            vec![
                json!("2024-01-15T10:30:00Z"),
                json!("2024-01-15T10:30:00+02:00"),
                json!("2024-01-15T10:30:00"),
                json!("2024-01-15T10:30Z"),
                json!("2024-01-15"),
                json!("2024-02-31T10:00:00Z"),
            ],
        ),
        case(
            "iso_time",
            vld::string().iso_time(),
            vec![json!("10:30:00"), json!("25:00:00")],
        ),
        case(
            "url",
            vld::string().url(),
            vec![
                json!("https://example.com/a"),
                json!("ftp://example.com"),
                json!("mailto:a@b.co"),
                json!("not a url"),
            ],
        ),
        case(
            "prefix_suffix",
            vld::string().starts_with("a.b").ends_with("z"),
            vec![json!("a.bxz"), json!("axbz"), json!("a.b")],
        ),
        case(
            "number_bounds",
            vld::number().min(0.0).lt(10.0),
            vec![json!(0), json!(9.99), json!(10), json!(-0.1), json!("1")],
        ),
        case(
            "int_exclusive",
            vld::number().int().positive().max(5),
            vec![json!(1), json!(5), json!(0), json!(6), json!(1.5)],
        ),
        case(
            "int_safe",
            vld::number().int().safe(),
            vec![json!(9007199254740991_i64), json!(9007199254740992_i64)],
        ),
        case(
            "multiple_of_float",
            vld::number().multiple_of(0.1),
            vec![json!(0.5), json!(0.3), json!(0.35)],
        ),
        case(
            "multiple_of_int",
            vld::number().int().multiple_of(3),
            vec![json!(9), json!(10)],
        ),
        case(
            "enum",
            vld::enumeration(&["a", "é"]),
            vec![json!("a"), json!("é"), json!("b"), json!(1)],
        ),
        case(
            "literal",
            vld::literal(5i64),
            vec![json!(5), json!(5.0), json!(6), json!("5")],
        ),
        case(
            "array",
            vld::array(vld::number().int())
                .min_len(1)
                .max_len(2)
                .unique(),
            vec![
                json!([1]),
                json!([1, 2]),
                json!([]),
                json!([1, 2, 3]),
                json!([1, 1]),
                json!(["x"]),
            ],
        ),
        case(
            "array_contains",
            vld::array(vld::number()).contains(5),
            vec![json!([1, 5]), json!([1, 2]), json!([])],
        ),
        case(
            "tuple",
            (vld::string(), vld::number()),
            vec![
                json!(["a", 1]),
                json!(["a"]),
                json!(["a", 1, 2]),
                json!([1, "a"]),
            ],
        ),
        case(
            "record",
            vld::record(vld::number().positive())
                .min_keys(1)
                .max_keys(2),
            vec![
                json!({"a": 1}),
                json!({}),
                json!({"a": -1}),
                json!({"a": 1, "b": 2, "c": 3}),
            ],
        ),
        case(
            "set_max_unique",
            vld::set(vld::number().int()).max_size(2),
            vec![json!([1, 2]), json!([1, 1, 2]), json!([1, 2, 3])],
        ),
        case(
            "object_strict",
            vld::object()
                .field("name", vld::string().min(1))
                .field("age", vld::number().int().optional())
                .strict(),
            vec![
                json!({"name": "a"}),
                json!({"name": "a", "age": null}),
                json!({"name": "a", "age": 3}),
                json!({"name": ""}),
                json!({}),
                json!({"name": "a", "x": 1}),
                json!({"name": "a", "age": "x"}),
            ],
        ),
        case(
            "object_catchall",
            vld::object()
                .field("a", vld::string())
                .catchall(vld::number()),
            vec![json!({"a": "x", "b": 1}), json!({"a": "x", "b": "y"})],
        ),
        case(
            "nullable",
            vld::string().min(2).nullable(),
            vec![json!(null), json!("ab"), json!("a"), json!(1)],
        ),
        case(
            "union",
            vld::union(vld::string().min(3), vld::number().min(10.0)),
            vec![json!("abc"), json!(10), json!("ab"), json!(3), json!(true)],
        ),
        case(
            "intersection",
            vld::intersection(vld::string().min(2), vld::string().max(3)),
            vec![json!("ab"), json!("a"), json!("abcd")],
        ),
        case(
            "discriminated",
            vld::discriminated_union("type")
                .variant_str(
                    "a",
                    vld::object()
                        .field("type", vld::literal("a"))
                        .field("n", vld::number()),
                )
                .variant_str("b", vld::object().field("type", vld::literal("b"))),
            vec![
                json!({"type": "a", "n": 1}),
                json!({"type": "a"}),
                json!({"type": "b"}),
                json!({"type": "c"}),
                json!({}),
            ],
        ),
        case(
            "bytes_base64",
            vld::bytes().base64().len(3),
            vec![json!("AQID"), json!("AQ==")],
        ),
    ]
}

#[test]
fn vld_and_jsonschema_agree() {
    let mut problems = Vec::new();
    let mut seen_known = Vec::new();
    for c in cases() {
        let validator = jsonschema::draft202012::options()
            .should_validate_formats(true)
            .build(&c.schema)
            .unwrap_or_else(|e| panic!("{}: emitted schema is invalid: {e}\n{}", c.name, c.schema));
        for sample in &c.samples {
            let vld = (c.vld_ok)(sample);
            let js = validator.is_valid(sample);
            let known = KNOWN_DIVERGENCES.iter().find(|(n, s, _, _)| {
                *n == c.name && serde_json::from_str::<Value>(s).unwrap() == *sample
            });
            match (vld == js, known) {
                (true, None) => {}
                (false, Some(&(_, _, expected_vld, _))) if expected_vld == vld => {
                    seen_known.push((c.name, sample.clone()));
                }
                (false, _) => problems.push(format!(
                    "{}: {sample} vld={vld} jsonschema={js}\n    schema: {}",
                    c.name, c.schema
                )),
                (true, Some(_)) => problems.push(format!(
                    "{}: {sample} is listed as a known divergence but both agree ({vld})",
                    c.name
                )),
            }
        }
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
    assert_eq!(seen_known.len(), KNOWN_DIVERGENCES.len(), "{seen_known:?}");
}
