//! Corpus: `validate_with_schema` keyword by keyword — a table of
//! `(schema, value, expected (path, code) issues)`; an empty issue list means
//! the value is valid.

use crate::common::verdict;
use serde_json::{Value, json};

type Row = (Value, Value, &'static [(&'static str, &'static str)]);

const OK: &[(&str, &str)] = &[];

/// A string schema with a date / date-time / time bound.
fn bound(format: &str, key: &str, value: &str) -> Value {
    json!({"type": "string", "format": format, key: value})
}

fn day(key: &str, value: &str) -> Value {
    bound("date", key, value)
}

fn instant(key: &str, value: &str) -> Value {
    bound("date-time", key, value)
}

fn clock(key: &str, value: &str) -> Value {
    bound("time", key, value)
}

/// `n` chained `$ref` hops ending in a string schema.
fn ref_chain(n: usize) -> Value {
    let defs: serde_json::Map<String, Value> = (0..n)
        .map(|i| {
            let next = if i + 1 == n {
                json!({"type": "string"})
            } else {
                json!({"$ref": format!("#/$defs/d{}", i + 1)})
            };
            (format!("d{i}"), next)
        })
        .collect();
    json!({"$defs": defs, "$ref": "#/$defs/d0"})
}

fn rows() -> Vec<Row> {
    let date_bounds = json!({"type": "string", "format": "date", "formatMinimum": "2020-01-01", "formatMaximum": "2020-12-31"});
    let dt_min =
        json!({"type": "string", "format": "date-time", "formatMinimum": "2020-01-01T10:00:00Z"});
    let dt_max = json!({"type": "string", "format": "date-time", "formatMaximum": "2020-01-01T10:00:00.5+00:00"});
    let time_max = json!({"type": "string", "format": "time", "formatMaximum": "10:00:00"});
    let time_min = json!({"type": "string", "format": "time", "formatMinimum": "09:30"});
    let obj = json!({
        "type": "object",
        "required": ["name", "age"],
        "properties": {
            "name": {"type": "string", "minLength": 1, "maxLength": 50},
            "age":  {"type": "integer", "minimum": 0, "maximum": 150}
        }
    });
    let nested = json!({
        "type": "object",
        "required": ["address"],
        "properties": {"address": {
            "type": "object", "required": ["city"],
            "properties": {"city": {"type": "string", "minLength": 1}}
        }}
    });
    let tuple = json!({"prefixItems": [{"type": "integer"}], "items": {"type": "string"}});
    let draft07_tuple =
        json!({"items": [{"type": "integer"}], "additionalItems": {"type": "string"}});
    let contains = json!({"contains": {"const": 3}, "minContains": 2, "maxContains": 3});
    let refs = json!({
        "$defs": {"Pos": {"type": "integer", "minimum": 1}, "a b": {"type": "string"}},
        "type": "object",
        "properties": {
            "n": {"$ref": "#/$defs/Pos", "maximum": 5},
            "s": {"$ref": "#/$defs/a%20b"}
        }
    });
    vec![
        // Boolean and non-object schemas.
        (json!(true), json!("anything"), OK),
        (json!(false), json!("anything"), &[("", "false_schema")]),
        (json!("not a schema"), json!(1), OK),
        // type
        (
            json!({"type": "string"}),
            json!(42),
            &[("", "invalid_type")],
        ),
        (json!({"type": ["string", "null"]}), json!(null), OK),
        (
            json!({"type": ["string", "null"]}),
            json!(1),
            &[("", "invalid_type")],
        ),
        (json!({"type": "integer"}), json!(2.0), OK),
        (
            json!({"type": "integer"}),
            json!(2.5),
            &[("", "invalid_type")],
        ),
        (json!({"type": "integer"}), json!(u64::MAX), OK),
        (json!({"type": "number"}), json!(-0.5), OK),
        (
            json!({"type": "boolean"}),
            json!(0),
            &[("", "invalid_type")],
        ),
        (
            json!({"type": "object"}),
            json!([]),
            &[("", "invalid_type")],
        ),
        (json!({"type": "array"}), json!({}), &[("", "invalid_type")]),
        (
            json!({"type": "null"}),
            json!(false),
            &[("", "invalid_type")],
        ),
        (
            json!({"type": "nonsense"}),
            json!(1),
            &[("", "invalid_type")],
        ),
        (json!({"type": 7}), json!(1), OK),
        // a type mismatch stops the other checks
        (
            json!({"type": "string", "minimum": 5}),
            json!(1),
            &[("", "invalid_type")],
        ),
        // strings: lengths count chars
        (
            json!({"type": "string", "minLength": 3}),
            json!("ab"),
            &[("", "too_small")],
        ),
        (json!({"type": "string", "minLength": 3}), json!("abc"), OK),
        (
            json!({"type": "string", "maxLength": 3}),
            json!("abcd"),
            &[("", "too_big")],
        ),
        (json!({"type": "string", "maxLength": 3}), json!("abc"), OK),
        (
            json!({"type": "string", "minLength": 3, "maxLength": 3}),
            json!("äöü"),
            OK,
        ),
        (
            json!({"type": "string", "minLength": 3, "maxLength": 3}),
            json!("äö"),
            &[("", "too_small")],
        ),
        (
            json!({"type": "string", "minLength": 3, "maxLength": 3}),
            json!("äöüß"),
            &[("", "too_big")],
        ),
        (json!({"minLength": 3}), json!(12), OK),
        (
            json!({"type": "string", "pattern": "^[a-z]+$"}),
            json!("hello"),
            OK,
        ),
        (
            json!({"type": "string", "pattern": "^[a-z]+$"}),
            json!("Hello123"),
            &[("", "invalid_string")],
        ),
        (
            json!({"type": "string", "pattern": "(unclosed"}),
            json!("x"),
            &[("", "invalid_pattern")],
        ),
        // formats
        (json!({"format": "email"}), json!("user@example.com"), OK),
        (json!({"format": "email"}), json!("a@b"), OK),
        (
            json!({"format": "email"}),
            json!("not-email"),
            &[("", "invalid_string")],
        ),
        (
            json!({"format": "email"}),
            json!("@b"),
            &[("", "invalid_string")],
        ),
        (
            json!({"format": "email"}),
            json!("a@"),
            &[("", "invalid_string")],
        ),
        (
            json!({"format": "email"}),
            json!("a b@c"),
            &[("", "invalid_string")],
        ),
        (json!({"format": "uri"}), json!("mailto:a@b.c"), OK),
        (json!({"format": "uri"}), json!("urn:isbn:123"), OK),
        (json!({"format": "uri"}), json!("ftp://x.y"), OK),
        (json!({"format": "uri"}), json!("a+b.c-d:x"), OK),
        (
            json!({"format": "uri"}),
            json!("no-scheme"),
            &[("", "invalid_string")],
        ),
        (
            json!({"format": "uri"}),
            json!("1http://x"),
            &[("", "invalid_string")],
        ),
        (
            json!({"format": "uri"}),
            json!("http://a b"),
            &[("", "invalid_string")],
        ),
        (
            json!({"format": "uri"}),
            json!("x:"),
            &[("", "invalid_string")],
        ),
        (
            json!({"format": "uri"}),
            json!("h_t:x"),
            &[("", "invalid_string")],
        ),
        (
            json!({"format": "uri"}),
            json!(":x"),
            &[("", "invalid_string")],
        ),
        (json!({"format": "url"}), json!("https://x"), OK),
        (json!({"format": "url"}), json!("http://x"), OK),
        (
            json!({"format": "url"}),
            json!("ftp://x"),
            &[("", "invalid_string")],
        ),
        (json!({"format": "url-strict"}), json!("http://x"), OK),
        (
            json!({"format": "url-strict"}),
            json!("x"),
            &[("", "invalid_string")],
        ),
        (
            json!({"format": "uuid"}),
            json!("9f3c4e7a-1b2d-4c3e-8f9a-0b1c2d3e4f5a"),
            OK,
        ),
        (
            json!({"format": "uuid"}),
            json!("9f3c4e7a-1b2d-4c3e-8f9a-0b1c2d3e4f5"),
            &[("", "invalid_string")],
        ),
        (
            json!({"format": "uuid"}),
            json!("9f3c4e7a-1b2d-4c3e-8f9a-0b1c2d3e4f5g"),
            &[("", "invalid_string")],
        ),
        (
            json!({"format": "uuid-v4"}),
            json!("9f3c4e7a-1b2d-4c3e-8f9a-0b1c2d3e4f5a"),
            OK,
        ),
        (
            json!({"format": "uuid-v7"}),
            json!("nope"),
            &[("", "invalid_string")],
        ),
        (
            json!({"format": "uuid-v1"}),
            json!("9f3c4e7a-1b2d-4c3e-8f9a-0b1c2d3e4f5z"),
            &[("", "invalid_string")],
        ),
        (json!({"format": "phone-e164"}), json!("+123"), OK),
        (
            json!({"format": "phone-e164"}),
            json!("123"),
            &[("", "invalid_string")],
        ),
        (json!({"format": "semver-full"}), json!("1.2.3"), OK),
        (
            json!({"format": "semver-full"}),
            json!("123"),
            &[("", "invalid_string")],
        ),
        (json!({"format": "ipv4"}), json!("1.2.3.4"), OK),
        (
            json!({"format": "ipv4"}),
            json!("256.1.1.1"),
            &[("", "invalid_string")],
        ),
        (json!({"format": "ipv6"}), json!("::1"), OK),
        (
            json!({"format": "ipv6"}),
            json!("1.2.3.4"),
            &[("", "invalid_string")],
        ),
        (json!({"format": "date"}), json!("2020-01-01"), OK),
        (
            json!({"format": "date"}),
            json!("2020/01/01"),
            &[("", "invalid_string")],
        ),
        (
            json!({"format": "date"}),
            json!("2020-0101"),
            &[("", "invalid_string")],
        ),
        (
            json!({"format": "date"}),
            json!("2020-01-011"),
            &[("", "invalid_string")],
        ),
        (
            json!({"format": "date"}),
            json!("2020-01/01"),
            &[("", "invalid_string")],
        ),
        (
            json!({"format": "date-time"}),
            json!("2020-01-01T00:00:00Z"),
            OK,
        ),
        (
            json!({"format": "date-time"}),
            json!("2020-01-01t00:00:00z"),
            OK,
        ),
        (
            json!({"format": "date-time"}),
            json!("2020-01-01 00:00"),
            &[("", "invalid_string")],
        ),
        (json!({"format": "country-code"}), json!("US"), OK),
        (
            json!({"format": "country-code"}),
            json!("USA"),
            &[("", "invalid_string")],
        ),
        (json!({"format": "currency-code"}), json!("USD"), OK),
        (
            json!({"format": "currency-code"}),
            json!("US"),
            &[("", "invalid_string")],
        ),
        (json!({"format": "locale"}), json!("en"), OK),
        (json!({"format": "locale"}), json!("en-US"), OK),
        (
            json!({"format": "locale"}),
            json!("eng"),
            &[("", "invalid_string")],
        ),
        (json!({"format": "no-such-format"}), json!("whatever"), OK),
        // formatMinimum / formatMaximum (inclusive) for date, date-time, time
        (date_bounds.clone(), json!("2020-06-01"), OK),
        (date_bounds.clone(), json!("2020-01-01"), OK),
        (date_bounds.clone(), json!("2020-12-31"), OK),
        (
            date_bounds.clone(),
            json!("2019-12-31"),
            &[("", "too_small")],
        ),
        (date_bounds.clone(), json!("2021-01-01"), &[("", "too_big")]),
        // day 30 of February rolls over to 1 March, inside the bounds
        (date_bounds.clone(), json!("2020-02-30"), OK),
        (
            date_bounds.clone(),
            json!("1900-03-01"),
            &[("", "too_small")],
        ),
        (date_bounds.clone(), json!("2400-02-29"), &[("", "too_big")]),
        (date_bounds, json!("2020-01-1x"), &[]),
        // 12:00+03:00 is 09:00Z: too early, although lexically larger.
        (
            dt_min.clone(),
            json!("2020-01-01T12:00:00+03:00"),
            &[("", "too_small")],
        ),
        (dt_min.clone(), json!("2020-01-01T10:00:00Z"), OK),
        (dt_min.clone(), json!("2020-01-01t10:00:00z"), OK),
        (
            dt_min.clone(),
            json!("2020-01-01 10:00:00"),
            &[("", "invalid_string")],
        ),
        (dt_min.clone(), json!("2020-01-01T08:00:00-03:00"), OK),
        (
            dt_min.clone(),
            json!("2020-01-01T09:59:59.999Z"),
            &[("", "too_small")],
        ),
        (
            dt_min.clone(),
            json!("2020-01-01T09:30:00-00:29"),
            &[("", "too_small")],
        ),
        (
            dt_min.clone(),
            json!("2019-12-31T23:59:59Z"),
            &[("", "too_small")],
        ),
        (dt_min, json!("2020-01-01T10"), OK),
        (dt_max.clone(), json!("2020-01-01T10:00:00.5Z"), OK),
        (
            dt_max.clone(),
            json!("2020-01-01T10:00:00.6Z"),
            &[("", "too_big")],
        ),
        (
            dt_max,
            json!("2020-01-01T13:00:00.6+03:00"),
            &[("", "too_big")],
        ),
        (time_max.clone(), json!("10:00"), OK),
        (time_max.clone(), json!("09:59:59.999999999"), OK),
        (time_max.clone(), json!("10:00:00.5"), &[("", "too_big")]),
        (time_max.clone(), json!("10:01"), &[("", "too_big")]),
        (time_max, json!("11"), OK),
        (time_min.clone(), json!("09:29:59"), &[("", "too_small")]),
        (time_min, json!("09:30:00"), OK),
        // adjacent days around month, leap-year, century and era boundaries
        (
            day("formatMinimum", "2020-03-01"),
            json!("2020-02-29"),
            &[("", "too_small")],
        ),
        (day("formatMinimum", "2020-02-29"), json!("2020-03-01"), OK),
        (
            day("formatMinimum", "2020-02-29"),
            json!("2020-02-28"),
            &[("", "too_small")],
        ),
        (
            day("formatMinimum", "2021-03-01"),
            json!("2021-02-28"),
            &[("", "too_small")],
        ),
        (day("formatMinimum", "2021-02-28"), json!("2021-03-01"), OK),
        (
            day("formatMinimum", "1900-03-01"),
            json!("1900-02-28"),
            &[("", "too_small")],
        ),
        (
            day("formatMinimum", "2000-03-01"),
            json!("2000-02-29"),
            &[("", "too_small")],
        ),
        (
            day("formatMinimum", "2000-01-01"),
            json!("1999-12-31"),
            &[("", "too_small")],
        ),
        (day("formatMinimum", "1999-12-31"), json!("2000-01-01"), OK),
        (
            day("formatMinimum", "2020-02-01"),
            json!("2020-01-31"),
            &[("", "too_small")],
        ),
        (
            day("formatMinimum", "2020-06-05"),
            json!("2020-06-04"),
            &[("", "too_small")],
        ),
        (day("formatMinimum", "2020-06-05"), json!("2020-06-05"), OK),
        (day("formatMinimum", "2020-06-05"), json!("2020-06-06"), OK),
        (
            day("formatMaximum", "2020-02-29"),
            json!("2020-03-01"),
            &[("", "too_big")],
        ),
        (
            day("formatMaximum", "2020-06-05"),
            json!("2020-06-06"),
            &[("", "too_big")],
        ),
        (
            day("formatMaximum", "2020-12-31"),
            json!("2021-01-01"),
            &[("", "too_big")],
        ),
        // one separator off: not a date, so no bound check
        (
            day("formatMinimum", "2020-01-01"),
            json!("2019/01-01"),
            &[("", "invalid_string")],
        ),
        (
            day("formatMinimum", "2020-01-01"),
            json!("2019-01/01"),
            &[("", "invalid_string")],
        ),
        // offsets that cross midnight and the leap day
        (
            instant("formatMinimum", "2020-02-29T23:45:00Z"),
            json!("2020-03-01T00:30:00+01:00"),
            &[("", "too_small")],
        ),
        (
            instant("formatMinimum", "2020-02-29T23:45:00Z"),
            json!("2020-03-01T00:50:00+01:00"),
            OK,
        ),
        (
            instant("formatMinimum", "2020-01-01T10:00:00Z"),
            json!("2020-01-01T09:30:00-00:31"),
            OK,
        ),
        (
            instant("formatMinimum", "2020-01-01T10:00:00Z"),
            json!("2020-01-01T09:30:00-00:29"),
            &[("", "too_small")],
        ),
        // not an offset: `+0300`, or a time ending in `:SS`
        (
            instant("formatMinimum", "2020-01-01T10:00:00Z"),
            json!("2020-01-01T09:00:00+0300"),
            OK,
        ),
        (
            instant("formatMaximum", "2020-01-01T10:00:00Z"),
            json!("2020-01-01T10:00:01"),
            &[("", "too_big")],
        ),
        (
            clock("formatMinimum", "09:30:30"),
            json!("09:30:10"),
            &[("", "too_small")],
        ),
        (clock("formatMinimum", "09:30:30"), json!("09:30:40"), OK),
        (
            clock("formatMaximum", "09:30:30.25"),
            json!("09:30:30.3"),
            &[("", "too_big")],
        ),
        // numbers
        (
            json!({"type": "number", "minimum": 10}),
            json!(5),
            &[("", "too_small")],
        ),
        (json!({"type": "number", "minimum": 10}), json!(10), OK),
        (json!({"type": "number", "minimum": 10}), json!(15), OK),
        (
            json!({"type": "number", "maximum": 100}),
            json!(150),
            &[("", "too_big")],
        ),
        (json!({"type": "number", "maximum": 100}), json!(100), OK),
        (
            json!({"type": "number", "exclusiveMinimum": 0, "exclusiveMaximum": 10}),
            json!(0),
            &[("", "too_small")],
        ),
        (
            json!({"type": "number", "exclusiveMinimum": 0, "exclusiveMaximum": 10}),
            json!(10),
            &[("", "too_big")],
        ),
        (
            json!({"type": "number", "exclusiveMinimum": 0, "exclusiveMaximum": 10}),
            json!(5),
            OK,
        ),
        (
            json!({"type": "number", "exclusiveMinimum": 0.5}),
            json!(0.25),
            &[("", "too_small")],
        ),
        (
            json!({"type": "number", "minimum": 1, "maximum": 0}),
            json!(0.5),
            &[("", "too_small"), ("", "too_big")],
        ),
        (json!({"minimum": "5"}), json!(1), OK),
        (json!({"type": "number", "multipleOf": 0.1}), json!(0.3), OK),
        (
            json!({"type": "number", "multipleOf": 0.1}),
            json!(0.35),
            &[("", "multipleOf")],
        ),
        (json!({"multipleOf": 3}), json!(-9), OK),
        (json!({"multipleOf": 3}), json!(10), &[("", "multipleOf")]),
        (json!({"multipleOf": 0}), json!(10), OK),
        (json!({"multipleOf": -2}), json!(3), OK),
        (json!({"multipleOf": 2}), json!(1e300), OK),
        (json!({"multipleOf": 0.5}), json!(2), OK),
        // tolerance grows with the quotient (100000000.30000001 / 0.1 is 1000000003.0000001)
        (json!({"multipleOf": 0.1}), json!(100000000.30000001), OK),
        (
            json!({"multipleOf": 0.1}),
            json!(100000000.35),
            &[("", "multipleOf")],
        ),
        // exact for large integers: 2^53 + 1 rounds to 2^53 as f64
        (
            json!({"type": "integer", "maximum": 9007199254740992u64}),
            json!(9007199254740993u64),
            &[("", "too_big")],
        ),
        (
            json!({"type": "integer", "minimum": u64::MAX}),
            json!(u64::MAX - 1),
            &[("", "too_small")],
        ),
        (
            json!({"type": "integer", "minimum": u64::MAX}),
            json!(u64::MAX),
            OK,
        ),
        (
            json!({"type": "integer", "maximum": i64::MIN}),
            json!(i64::MIN),
            OK,
        ),
        // 2^53 + 3 rounds to 2^53 + 4 (a multiple of 3) as f64
        (
            json!({"multipleOf": 3}),
            json!(9007199254740995u64),
            &[("", "multipleOf")],
        ),
        (json!({"multipleOf": 3}), json!(9007199254740993u64), OK),
        // arrays
        (
            json!({"type": "array", "minItems": 2}),
            json!([1]),
            &[("", "too_small")],
        ),
        (json!({"type": "array", "minItems": 2}), json!([1, 2]), OK),
        (
            json!({"type": "array", "maxItems": 1}),
            json!([1, 2]),
            &[("", "too_big")],
        ),
        (json!({"type": "array", "maxItems": 1}), json!([1]), OK),
        (
            json!({"type": "array", "items": {"type": "integer", "minimum": 0}}),
            json!([1, 2, 3]),
            OK,
        ),
        (
            json!({"type": "array", "items": {"type": "integer", "minimum": 0}}),
            json!([1, -1, -3]),
            &[("[1]", "too_small"), ("[2]", "too_small")],
        ),
        (tuple.clone(), json!([1, "a", "b"]), OK),
        (tuple.clone(), json!([1, 2]), &[("[1]", "invalid_type")]),
        (tuple, json!(["x"]), &[("[0]", "invalid_type")]),
        (draft07_tuple.clone(), json!([1, "a"]), OK),
        (
            draft07_tuple.clone(),
            json!([1, 2]),
            &[("[1]", "invalid_type")],
        ),
        (
            json!({"items": [{"type": "integer"}]}),
            json!([1, "free"]),
            OK,
        ),
        (
            json!({"prefixItems": [{"type": "integer"}], "items": false}),
            json!([1, 2]),
            &[("[1]", "false_schema")],
        ),
        (json!({"uniqueItems": true}), json!([1, 2, {"a": 1}]), OK),
        (
            json!({"uniqueItems": true}),
            json!([1, 2, 1, 2]),
            &[("[2]", "uniqueItems")],
        ),
        (json!({"uniqueItems": false}), json!([1, 1]), OK),
        // numbers compare by value: 2 and 2.0 are the same item
        (
            json!({"uniqueItems": true}),
            json!([2, 2.0]),
            &[("[1]", "uniqueItems")],
        ),
        (
            json!({"uniqueItems": true}),
            json!([9007199254740993_u64, 9007199254740992.0]),
            OK,
        ),
        (json!({"enum": [1.0, "a"]}), json!(1), OK),
        (json!({"const": 2}), json!(2.0), OK),
        (
            json!({"const": 9007199254740993_u64}),
            json!(9007199254740992.0),
            &[("", "const")],
        ),
        (contains.clone(), json!([3, 3]), OK),
        (contains.clone(), json!([3, 3, 3, 1]), OK),
        (contains.clone(), json!([3]), &[("", "contains")]),
        (contains, json!([3, 3, 3, 3]), &[("", "contains")]),
        // default minContains is 1
        (json!({"contains": {"const": "x"}}), json!(["x"]), OK),
        (
            json!({"contains": {"const": "x"}}),
            json!([]),
            &[("", "contains")],
        ),
        (
            json!({"contains": {"const": "x"}, "maxContains": 1}),
            json!(["x", "x"]),
            &[("", "contains")],
        ),
        // objects; properties are visited in key order
        (obj.clone(), json!({"name": "Alice", "age": 30}), OK),
        (
            obj.clone(),
            json!({"name": "", "age": 151}),
            &[(".age", "too_big"), (".name", "too_small")],
        ),
        (
            obj,
            json!({}),
            &[(".name", "missing_field"), (".age", "missing_field")],
        ),
        (nested.clone(), json!({"address": {"city": "NYC"}}), OK),
        (
            nested.clone(),
            json!({"address": {"city": ""}}),
            &[(".address.city", "too_small")],
        ),
        (
            nested,
            json!({"address": {}}),
            &[(".address.city", "missing_field")],
        ),
        (
            json!({"required": [1, "a"]}),
            json!({}),
            &[(".a", "missing_field")],
        ),
        (
            json!({"additionalProperties": false, "properties": {"a": {}}}),
            json!({"a": 1, "b": 2}),
            &[(".b", "unrecognized_field")],
        ),
        (
            json!({"additionalProperties": {"type": "integer"}}),
            json!({"a": 1}),
            OK,
        ),
        (
            json!({"additionalProperties": {"type": "integer"}}),
            json!({"a": "x"}),
            &[(".a", "invalid_type")],
        ),
        (json!({"additionalProperties": true}), json!({"a": "x"}), OK),
        (
            json!({"patternProperties": {"^n_": {"type": "integer"}}, "additionalProperties": false}),
            json!({"n_a": 1}),
            OK,
        ),
        (
            json!({"patternProperties": {"^n_": {"type": "integer"}}, "additionalProperties": false}),
            json!({"b": 1, "n_a": "x"}),
            &[(".b", "unrecognized_field"), (".n_a", "invalid_type")],
        ),
        (
            json!({"properties": {"n_a": {"minimum": 5}}, "patternProperties": {"^n_": {"maximum": 1}}}),
            json!({"n_a": 3}),
            &[(".n_a", "too_small"), (".n_a", "too_big")],
        ),
        (
            json!({"patternProperties": {"(": {}}}),
            json!({"a": 1}),
            &[("", "invalid_pattern")],
        ),
        (
            json!({"propertyNames": {"minLength": 2}}),
            json!({"ab": 1}),
            OK,
        ),
        (
            json!({"propertyNames": {"minLength": 2}}),
            json!({"a": 1}),
            &[(".a", "too_small")],
        ),
        (json!({"minProperties": 1}), json!({}), &[("", "too_small")]),
        (json!({"minProperties": 1}), json!({"a": 1}), OK),
        (
            json!({"maxProperties": 1}),
            json!({"a": 1, "b": 2}),
            &[("", "too_big")],
        ),
        (json!({"maxProperties": 1}), json!({"a": 1}), OK),
        // enum / const
        (json!({"enum": ["red", "green", "blue"]}), json!("red"), OK),
        (
            json!({"enum": ["red", "green", "blue"]}),
            json!("yellow"),
            &[("", "enum")],
        ),
        (json!({"const": {"a": [1]}}), json!({"a": [1]}), OK),
        (json!({"const": null}), json!(0), &[("", "const")]),
        // combinators
        (
            json!({"anyOf": [{"type": "string"}, {"type": "integer"}]}),
            json!("hello"),
            OK,
        ),
        (
            json!({"anyOf": [{"type": "string"}, {"type": "integer"}]}),
            json!(42),
            OK,
        ),
        (
            json!({"anyOf": [{"type": "string"}, {"type": "integer"}]}),
            json!(true),
            &[("", "anyOf")],
        ),
        (
            json!({"anyOf": [{"type": "string"}, {"type": "null"}]}),
            json!(null),
            OK,
        ),
        (
            json!({"oneOf": [{"type": "integer"}, {"minimum": 0}]}),
            json!(-1),
            OK,
        ),
        (
            json!({"oneOf": [{"type": "integer"}, {"minimum": 0}]}),
            json!(1),
            &[("", "oneOf")],
        ),
        (
            json!({"oneOf": [{"type": "integer"}, {"minimum": 0}]}),
            json!(-0.5),
            &[("", "oneOf")],
        ),
        (
            json!({"allOf": [{"type": "number", "minimum": 0}, {"type": "number", "maximum": 100}]}),
            json!(50),
            OK,
        ),
        (
            json!({"allOf": [{"type": "number", "minimum": 0}, {"type": "number", "maximum": 100}]}),
            json!(150),
            &[("", "too_big")],
        ),
        (
            json!({"allOf": [{"type": "number", "minimum": 0}, {"type": "number", "maximum": 100}]}),
            json!(-1),
            &[("", "too_small")],
        ),
        (json!({"not": {"type": "string"}}), json!(42), OK),
        (
            json!({"not": {"type": "string"}}),
            json!("hello"),
            &[("", "not")],
        ),
        // $ref: siblings apply, percent-decoded names, self refs and dangling refs
        (refs.clone(), json!({"n": 3, "s": "x"}), OK),
        (
            refs.clone(),
            json!({"n": 0, "s": 1}),
            &[(".n", "too_small"), (".s", "invalid_type")],
        ),
        (refs, json!({"n": 6}), &[(".n", "too_big")]),
        (ref_chain(64), json!("s"), OK),
        (ref_chain(65), json!("s"), &[("", "ref_depth")]),
        (json!({"$ref": "#"}), json!(1), &[("", "ref_depth")]),
        (
            json!({"$ref": "#/$defs/Missing"}),
            json!(1),
            &[("", "invalid_ref")],
        ),
        (
            json!({"$ref": "other.json#/x"}),
            json!(1),
            &[("", "invalid_ref")],
        ),
        (
            json!({"$defs": {"%zz": {"type": "string"}}, "$ref": "#/$defs/%zz"}),
            json!(1),
            &[("", "invalid_type")],
        ),
    ]
}

/// Given each row's schema and value, when `validate_with_schema` runs, then
/// it reports exactly the expected `(path, code)` issues (none = valid).
#[test]
fn every_keyword_gives_the_expected_issues() {
    let mut failures = Vec::new();
    for (schema, value, expected) in rows() {
        let got = verdict(&schema, &value).err().unwrap_or_default();
        let want: Vec<(String, String)> = expected
            .iter()
            .map(|(p, c)| (p.to_string(), c.to_string()))
            .collect();
        if got != want {
            failures.push(format!(
                "{value} against {schema}: got {got:?}, want {want:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Given a string failing each known `format`, or a value of each JSON type
/// failing a `type`, when it is validated, then the issue code names the
/// exact string check, or the expected and received types.
#[test]
fn issue_codes_name_the_failed_check() {
    use vld::error::{IssueCode, StringValidation as V};
    let formats = [
        ("email", "x", V::Email),
        ("uri", "x", V::Url),
        ("url", "x", V::Url),
        ("url-strict", "x", V::UrlStrict),
        ("uuid", "x", V::Uuid),
        ("uuid-v1", "x", V::UuidV1),
        ("uuid-v4", "x", V::UuidV4),
        ("uuid-v7", "x", V::UuidV7),
        ("phone-e164", "x", V::PhoneE164),
        ("semver-full", "x", V::SemverFull),
        ("ipv4", "x", V::Ipv4),
        ("ipv6", "x", V::Ipv6),
        ("date", "x", V::IsoDate),
        ("date-time", "x", V::IsoDatetime),
        ("country-code", "x", V::CountryCode),
        ("currency-code", "x", V::Currency),
        ("locale", "x", V::Locale),
    ];
    for (format, bad, want) in formats {
        let err =
            vld_openapi::validator::validate_with_schema(&json!({"format": format}), &json!(bad))
                .unwrap_err();
        assert_eq!(
            err.issues[0].code,
            IssueCode::InvalidString { validation: want },
            "{format}"
        );
    }
    let pattern =
        vld_openapi::validator::validate_with_schema(&json!({"pattern": "^a$"}), &json!("b"))
            .unwrap_err();
    assert_eq!(
        pattern.issues[0].code,
        IssueCode::InvalidString {
            validation: V::Regex
        }
    );

    for (value, received) in [
        (json!(null), "null"),
        (json!(true), "boolean"),
        (json!(1), "integer"),
        (json!(u64::MAX), "integer"),
        (json!(1.5), "number"),
        (json!("s"), "string"),
        (json!([]), "array"),
        (json!({}), "object"),
    ] {
        let schema = if received == "null" {
            json!({"type": ["integer", "string"]})
        } else {
            json!({"type": "null"})
        };
        let err = vld_openapi::validator::validate_with_schema(&schema, &value).unwrap_err();
        let expected = if received == "null" {
            "integer | string"
        } else {
            "null"
        };
        assert_eq!(
            err.issues[0].code,
            IssueCode::InvalidType {
                expected: expected.into(),
                received: received.into()
            },
            "{value}"
        );
    }
}
