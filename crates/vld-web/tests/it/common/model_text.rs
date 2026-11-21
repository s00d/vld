//! Models of the text → JSON rules shared by every adapter.
//!
//! - `coerce_value`: random number-like, boolean-like and arbitrary strings.
//!   The model is the documented rule: a value is converted only when the
//!   conversion prints back to exactly the same text (`true`/`false`, `i64`,
//!   `u64`, finite decimals with a `.`), otherwise it stays a string; and
//!   every result prints back to the input.
//! - `parse_query_string` / `parse_form_body`: random key/value lists, form
//!   encoded with random escapes, `[]` suffixes and repeats. The model
//!   predicts the whole map (arrays for repeats and `[]`, coerced values).
//! - `percent_decode` / `url_decode`: any text, escaped byte by byte at
//!   random, decodes back to itself.

use proptest::prelude::*;
use serde_json::{Map, Value};
use vld_web::common::*;

/// The text a coerced value stands for.
fn text_of(v: &Value) -> String {
    match v {
        Value::Bool(b) => b.to_string(),
        Value::String(s) => s.clone(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(u) = n.as_u64() {
                u.to_string()
            } else {
                n.as_f64().unwrap().to_string()
            }
        }
        other => panic!("coerce_value produced {other}"),
    }
}

/// The documented rule.
fn model_coerce(raw: &str) -> Value {
    match raw {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }
    if let Ok(i) = raw.parse::<i64>()
        && i.to_string() == raw
    {
        return i.into();
    }
    if let Ok(u) = raw.parse::<u64>()
        && u.to_string() == raw
    {
        return u.into();
    }
    if raw.contains('.')
        && let Ok(f) = raw.parse::<f64>()
        && f.is_finite()
        && f.to_string() == raw
    {
        return f.into();
    }
    Value::String(raw.to_string())
}

fn number_like() -> impl Strategy<Value = String> {
    let canonical = prop_oneof![
        any::<i64>().prop_map(|i| i.to_string()),
        any::<u64>().prop_map(|u| u.to_string()),
        (0u64..1000).prop_map(|u| u.to_string()),
        any::<f64>()
            .prop_filter("finite", |f| f.is_finite())
            .prop_map(|f| f.to_string()),
        (-1000i32..1000, 1u32..10_000).prop_map(|(i, f)| format!("{i}.{f}")),
        "-?[0-9]{1,25}(\\.[0-9]{1,25})?",
    ];
    let noise = prop_oneof![
        Just(""),
        Just("+"),
        Just("-"),
        Just("0"),
        Just("."),
        Just(" "),
        Just("e3"),
        Just("E-2"),
        Just(".0"),
    ];
    (canonical, noise.clone(), noise, 0..3u8).prop_map(|(n, pre, post, mode)| match mode {
        0 => n,
        1 => format!("{pre}{n}"),
        _ => format!("{n}{post}"),
    })
}

fn raw_text() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => number_like(),
        1 => prop_oneof![
            Just("true".to_string()),
            Just("false".to_string()),
            Just("TRUE".to_string()),
            Just("True".to_string()),
            Just("null".to_string()),
            Just("-0".to_string()),
            Just("-0.0".to_string()),
            Just("inf".to_string()),
            Just("NaN".to_string()),
        ],
        1 => "[0-9a-z.+\\- é]{0,8}",
        1 => any::<String>(),
    ]
}

/// One form field: key, value, `[]` suffix.
type Field = (String, String, bool);

fn field() -> impl Strategy<Value = Field> {
    // A few keys so repeats are common; keys may contain anything that must
    // be escaped (`&`, `=`, `+`, `%`, space, non-ASCII), but no brackets.
    let key = prop_oneof![
        3 => prop::sample::select(vec!["a", "b", "tag", "x y", "k&=", "é", ""])
            .prop_map(str::to_string),
        1 => "[a-z&=+% é]{0,4}",
    ];
    (key, raw_text(), any::<bool>())
}

/// Form-encode `s`, escaping each safe byte at random (the `bool`s).
fn encode(s: &str, escape: &[bool]) -> String {
    let mut out = String::new();
    for (i, b) in s.bytes().enumerate() {
        let forced = !(b.is_ascii_alphanumeric() || b"-._~".contains(&b));
        if b == b' ' && !escape.get(i).copied().unwrap_or(false) {
            out.push('+');
        } else if forced || escape.get(i).copied().unwrap_or(false) {
            out.push_str(&format!("%{b:02X}"));
        } else {
            out.push(b as char);
        }
    }
    out
}

fn model_query(fields: &[Field]) -> Map<String, Value> {
    let mut order: Vec<(String, Vec<Value>, bool)> = Vec::new();
    for (k, v, forced) in fields {
        let value = model_coerce(v);
        match order.iter_mut().find(|(key, _, _)| key == k) {
            Some((_, values, _)) => values.push(value),
            None => order.push((k.clone(), vec![value], *forced)),
        }
    }
    order
        .into_iter()
        .map(|(k, mut values, forced)| {
            let v = if values.len() == 1 && !forced {
                values.pop().unwrap()
            } else {
                Value::Array(values)
            };
            (k, v)
        })
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn coerce_follows_the_round_trip_rule(raw in raw_text()) {
        let got = coerce_value(&raw);
        prop_assert_eq!(&got, &model_coerce(&raw));
        // Never loses information.
        prop_assert_eq!(text_of(&got), raw);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn query_and_form_bodies_follow_the_model(
        fields in prop::collection::vec(field(), 0..8),
        escapes in prop::collection::vec(any::<bool>(), 64),
        empty_pairs in prop::collection::vec(any::<bool>(), 8),
    ) {
        let mut parts = Vec::new();
        for (i, (k, v, forced)) in fields.iter().enumerate() {
            if empty_pairs[i] {
                parts.push(String::new()); // `&&` is skipped
            }
            let key = format!("{}{}", encode(k, &escapes), if *forced { "[]" } else { "" });
            // `?flag` (no `=`) reads as the empty value.
            if v.is_empty() && escapes[i] && !key.is_empty() {
                parts.push(key);
            } else {
                parts.push(format!("{key}={}", encode(v, &escapes[8..])));
            }
        }
        let query = parts.join("&");
        let want = model_query(&fields);

        prop_assert_eq!(&parse_query_string(&query), &want, "query {:?}", query);
        prop_assert_eq!(query_string_to_json(&query), Value::Object(want.clone()));
        prop_assert_eq!(parse_form_body(query.as_bytes()).unwrap(), Value::Object(want));
    }

    #[test]
    fn decoding_inverts_encoding(
        s in any::<String>(),
        escapes in prop::collection::vec(any::<bool>(), 0..64),
    ) {
        let form = encode(&s, &escapes);
        prop_assert_eq!(url_decode(&form), s.clone());
        // `percent_decode` keeps `+`: escape spaces as `%20` for it.
        let all: Vec<bool> = s.bytes().map(|b| b == b' ').chain(escapes).collect();
        prop_assert_eq!(percent_decode(&encode(&s, &all)), s);
    }
}
