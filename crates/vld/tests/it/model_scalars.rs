//! Model tests: scalar and system values whose rules are arithmetic — `bytes()` in every
//! encoding, `decimal()` bounds, `duration()` bounds, `socket_addr()` ports, `json_value()`
//! depth and shape, JSON equality (`contains`, `literal`) and the `IntoLiteral` conversions.
//!
//! proptest picks the options and an input; the model predicts the output or the exact issue
//! codes (and, with `openapi`, the exact JSON Schema).

use crate::common::*;

use proptest::prelude::*;
use serde_json::{Value, json};
use vld::error::{IssueCode, SizeOrigin};
use vld::prelude::*;

// ---------------------------------------------------------------- bytes

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mode {
    Off,
    Base64,
    Base64Url,
    Hex,
}

/// How the input carries the data.
#[derive(Debug, Clone, Copy)]
enum Repr {
    /// A JSON array of byte values; `Some(i)` puts 256 at index `i`.
    Array(Option<usize>),
    /// Encoded in the schema's string mode (Base64 when the mode is off);
    /// `pad` / `upper` / `prefix` pick the spelling.
    Encoded {
        pad: bool,
        upper: bool,
        prefix: bool,
    },
    /// A JSON number.
    Number,
}

fn base64(data: &[u8], url: bool, pad: bool) -> String {
    let alphabet: &[u8; 64] = if url {
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
    } else {
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
    };
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = |i: usize| u32::from(chunk.get(i).copied().unwrap_or(0));
        let v = (b(0) << 16) | (b(1) << 8) | b(2);
        let sym = |shift: u32| alphabet[((v >> shift) & 63) as usize] as char;
        out.push(sym(18));
        out.push(sym(12));
        for (needed, shift) in [(2, 6), (3, 0)] {
            if chunk.len() >= needed {
                out.push(sym(shift));
            } else if pad {
                out.push('=');
            }
        }
    }
    out
}

fn hex(data: &[u8], upper: bool, prefix: bool) -> String {
    let digits: String = data
        .iter()
        .map(|b| {
            if upper {
                format!("{b:02X}")
            } else {
                format!("{b:02x}")
            }
        })
        .collect();
    if prefix {
        format!("0x{digits}")
    } else {
        digits
    }
}

fn bytes_schema(mode: Mode, min: Option<usize>, max: Option<usize>, len: Option<usize>) -> ZBytes {
    let mut s = vld::bytes();
    s = match mode {
        Mode::Off => s,
        Mode::Base64 => s.base64(),
        Mode::Base64Url => s.base64url(),
        Mode::Hex => s.hex(),
    };
    if let Some(n) = min {
        s = s.min_len(n);
    }
    if let Some(n) = max {
        s = s.max_len(n);
    }
    if let Some(n) = len {
        s = s.len(n);
    }
    s
}

/// The JSON Schema the docs describe: `minItems`/`maxItems` (strictest bound) on the array, and in
/// string modes the encoded lengths — Base64 `4*ceil(n/3)`, Base64url unpadded minimum, hex `2n`
/// (plus 2 for `0x` on the maximum).
#[cfg(feature = "openapi")]
fn bytes_json_schema(
    mode: Mode,
    min: Option<usize>,
    max: Option<usize>,
    len: Option<usize>,
) -> Value {
    let lo = [min, len].into_iter().flatten().max();
    let hi = [max, len].into_iter().flatten().min();
    let mut array = json!({
        "type": "array",
        "items": { "type": "integer", "minimum": 0, "maximum": 255 }
    });
    if let Some(n) = lo {
        array["minItems"] = json!(n);
    }
    if let Some(n) = hi {
        array["maxItems"] = json!(n);
    }
    let padded = |n: usize| n.div_ceil(3) * 4;
    let (format, enc_lo, enc_hi) = match mode {
        Mode::Off => return array,
        Mode::Base64 => ("byte", lo.map(padded), hi.map(padded)),
        Mode::Base64Url => (
            "byte-base64url",
            lo.map(|n| (n * 4).div_ceil(3)),
            hi.map(padded),
        ),
        Mode::Hex => ("byte-hex", lo.map(|n| 2 * n), hi.map(|n| 2 * n + 2)),
    };
    let mut string = json!({ "type": "string", "format": format });
    if let Some(n) = enc_lo {
        string["minLength"] = json!(n);
    }
    if let Some(n) = enc_hi {
        string["maxLength"] = json!(n);
    }
    json!({ "anyOf": [string, array] })
}

fn byte() -> impl Strategy<Value = u8> {
    prop_oneof![Just(0u8), Just(255u8), any::<u8>()]
}

fn opt_len() -> impl Strategy<Value = Option<usize>> {
    prop::option::of(0usize..7)
}

fn repr() -> impl Strategy<Value = Repr> {
    prop_oneof![
        prop::option::of(0usize..8).prop_map(Repr::Array),
        (any::<bool>(), any::<bool>(), any::<bool>())
            .prop_map(|(pad, upper, prefix)| Repr::Encoded { pad, upper, prefix }),
        Just(Repr::Number),
    ]
}

fn mode() -> impl Strategy<Value = Mode> {
    prop_oneof![
        Just(Mode::Off),
        Just(Mode::Base64),
        Just(Mode::Base64Url),
        Just(Mode::Hex)
    ]
}

proptest! {
    /// Given `bytes()` in a random mode with random `min_len` / `max_len` / `len`
    /// When random data arrives as a byte array (maybe with a 256 in it), encoded in the mode's
    /// alphabet (padded or not, either hex case, with or without `0x`) or as a number
    /// Then the decoded bytes come back; 256 is `too_big` (maximum 255) at its index; a string in
    /// array mode and a number are `invalid_type` (expected `bytes`, or `bytes_or_string` in a
    /// string mode); every length rule that fails is reported, in order, as a `bytes` size; and the
    /// JSON Schema is exactly the documented one.
    #[test]
    fn bytes_model(
        mode in mode(),
        min in opt_len(),
        max in opt_len(),
        len in opt_len(),
        data in prop::collection::vec(byte(), 0..8),
        repr in repr(),
    ) {
        let schema = bytes_schema(mode, min, max, len);
        let expected_type = if mode == Mode::Off { "bytes" } else { "bytes_or_string" };
        let (input, decoded): (Value, Option<Vec<u8>>) = match repr {
            Repr::Array(bump) => {
                let mut items: Vec<Value> = data.iter().map(|&b| json!(b)).collect();
                let bad = bump.filter(|&i| i < items.len());
                if let Some(i) = bad {
                    items[i] = json!(256);
                }
                let arr = Value::Array(items);
                if let Some(i) = bad {
                    let err = schema.parse_value(&arr).unwrap_err();
                    prop_assert_eq!(issues(&err), vec![(format!("[{i}]"), "too_big".to_string())]);
                    prop_assert_eq!(&err.issues[0].code, &IssueCode::TooBig {
                        origin: SizeOrigin::Number,
                        maximum: json!(255),
                        inclusive: true,
                    });
                    return Ok(());
                }
                (arr, Some(data.clone()))
            }
            Repr::Encoded { pad, upper, prefix } => {
                let text = match mode {
                    Mode::Off | Mode::Base64 => base64(&data, false, true),
                    Mode::Base64Url => base64(&data, true, pad),
                    Mode::Hex => hex(&data, upper, prefix),
                };
                // An empty Base64 / hex string is malformed (covered by the corpus).
                if data.is_empty() && mode != Mode::Off {
                    return Ok(());
                }
                let ok = mode != Mode::Off;
                (json!(text), ok.then(|| data.clone()))
            }
            Repr::Number => (json!(7), None),
        };
        let Some(decoded) = decoded else {
            let err = schema.parse_value(&input).unwrap_err();
            let received = if input.is_string() { "string" } else { "number" };
            prop_assert_eq!(&err.issues[0].code, &IssueCode::InvalidType {
                expected: expected_type.into(),
                received: received.into(),
            });
            prop_assert_eq!(err.issues.len(), 1);
            return Ok(());
        };

        let n = decoded.len();
        let mut want = Vec::new();
        if let Some(m) = min.filter(|&m| n < m) {
            want.push(IssueCode::TooSmall { origin: SizeOrigin::Bytes, minimum: m.into(), inclusive: true });
        }
        if let Some(m) = max.filter(|&m| n > m) {
            want.push(IssueCode::TooBig { origin: SizeOrigin::Bytes, maximum: m.into(), inclusive: true });
        }
        if let Some(m) = len.filter(|&m| n != m) {
            want.push(IssueCode::InvalidLength { origin: SizeOrigin::Bytes, expected: m });
        }
        match schema.parse_value(&input) {
            Ok(out) => {
                prop_assert!(want.is_empty(), "{input} accepted, want {want:?}");
                prop_assert_eq!(out, decoded);
            }
            Err(err) => {
                let got: Vec<IssueCode> = err.issues.iter().map(|i| i.code.clone()).collect();
                prop_assert_eq!(got, want, "{}", input);
            }
        }

        #[cfg(feature = "openapi")]
        {
            use vld::openapi::JsonSchema;
            prop_assert_eq!(schema.json_schema(), bytes_json_schema(mode, min, max, len));
        }
    }
}

// ---------------------------------------------------------------- decimal

/// A multiple of 0.25 written with two fraction digits (`-1.25`, `0.50`).
#[cfg(feature = "decimal")]
fn dec(quarters: i32) -> String {
    let sign = if quarters < 0 { "-" } else { "" };
    let a = quarters.abs();
    format!("{sign}{}.{:02}", a / 4, (a % 4) * 25)
}

#[cfg(feature = "decimal")]
#[derive(Debug, Clone)]
struct DecChecks {
    min: Option<i32>,
    max: Option<i32>,
    gt: Option<i32>,
    lt: Option<i32>,
    multiple_of: Option<i32>,
    positive: bool,
    negative: bool,
    non_negative: bool,
    non_positive: bool,
}

#[cfg(feature = "decimal")]
fn dec_checks() -> impl Strategy<Value = DecChecks> {
    // Few checks at a time, so the sign checks are often the deciding one.
    let q = || prop::option::weighted(0.3, -6i32..=6);
    let flag = || prop::bool::weighted(0.3);
    (
        (q(), q(), q(), q()),
        prop::option::of(prop_oneof![Just(0), Just(2), Just(4)]),
        (flag(), flag(), flag(), flag()),
    )
        .prop_map(
            |((min, max, gt, lt), multiple_of, (p, n, nn, np))| DecChecks {
                min,
                max,
                gt,
                lt,
                multiple_of,
                positive: p,
                negative: n,
                non_negative: nn,
                non_positive: np,
            },
        )
}

/// The strictest of `(value in quarters, display, exclusive)` bounds: the larger (for a lower
/// bound) wins, and on a tie a later exclusive bound replaces the earlier one.
#[cfg(all(feature = "decimal", feature = "openapi"))]
fn strictest(bounds: Vec<(i32, String, bool)>, lower: bool) -> Option<(i32, String, bool)> {
    bounds.into_iter().reduce(|a, b| {
        let beyond = if lower { b.0 > a.0 } else { b.0 < a.0 };
        if beyond || (b.0 == a.0 && b.2) { b } else { a }
    })
}

#[cfg(feature = "decimal")]
proptest! {
    /// Given `decimal()` with random `min` / `max` / `gt` / `lt` / `multiple_of` (including 0)
    /// and sign checks
    /// When a decimal string (a multiple of 0.25) is parsed
    /// Then the first failing check in the documented order is reported — `too_small` /
    /// `too_big` with a `decimal` origin, the bound as written and its inclusiveness, sign checks
    /// against `0`, `not_multiple_of` naming the step — or the value keeps its scale; and the JSON
    /// Schema carries the strictest lower / upper bound (sign checks included) and the step.
    #[test]
    fn decimal_model(c in dec_checks(), k in prop_oneof![Just(0), -8i32..=8]) {
        let mut s = vld::decimal();
        if let Some(v) = c.min { s = s.min(dec(v)); }
        if let Some(v) = c.max { s = s.max(dec(v)); }
        if let Some(v) = c.gt { s = s.gt(dec(v)); }
        if let Some(v) = c.lt { s = s.lt(dec(v)); }
        if let Some(v) = c.multiple_of { s = s.multiple_of(dec(v)); }
        if c.positive { s = s.positive(); }
        if c.negative { s = s.negative(); }
        if c.non_negative { s = s.non_negative(); }
        if c.non_positive { s = s.non_positive(); }

        let small = |b: String, inclusive| IssueCode::TooSmall { origin: SizeOrigin::Decimal, minimum: json!(b), inclusive };
        let big = |b: String, inclusive| IssueCode::TooBig { origin: SizeOrigin::Decimal, maximum: json!(b), inclusive };
        let zero = || "0".to_string();
        let want = [
            c.min.filter(|&m| k < m).map(|m| small(dec(m), true)),
            c.max.filter(|&m| k > m).map(|m| big(dec(m), true)),
            c.gt.filter(|&m| k <= m).map(|m| small(dec(m), false)),
            c.lt.filter(|&m| k >= m).map(|m| big(dec(m), false)),
            c.multiple_of
                .filter(|&m| m == 0 || k % m != 0)
                .map(|m| IssueCode::NotMultipleOf { divisor: json!(dec(m)) }),
            (c.positive && k <= 0).then(|| small(zero(), false)),
            (c.negative && k >= 0).then(|| big(zero(), false)),
            (c.non_negative && k < 0).then(|| small(zero(), true)),
            (c.non_positive && k > 0).then(|| big(zero(), true)),
        ]
        .into_iter()
        .flatten()
        .next();

        let input = json!(dec(k));
        match (s.parse_value(&input), want) {
            (Ok(out), None) => prop_assert_eq!(out.to_string(), dec(k)),
            (Err(err), Some(code)) => {
                prop_assert_eq!(err.issues.len(), 1);
                prop_assert_eq!(&err.issues[0].code, &code);
            }
            (got, want) => prop_assert!(false, "{input}: got {got:?}, want {want:?}"),
        }

        #[cfg(feature = "openapi")]
        {
            use vld::openapi::JsonSchema;
            let mut want = json!({ "type": ["string", "number"], "format": "decimal" });
            let lower = strictest(
                [
                    c.min.map(|v| (v, dec(v), false)),
                    c.gt.map(|v| (v, dec(v), true)),
                    c.positive.then(|| (0, zero(), true)),
                    c.non_negative.then(|| (0, zero(), false)),
                ]
                .into_iter()
                .flatten()
                .collect(),
                true,
            );
            if let Some((_, v, excl)) = lower {
                want[if excl { "x-exclusiveMinDecimal" } else { "x-minDecimal" }] = json!(v);
            }
            let upper = strictest(
                [
                    c.max.map(|v| (v, dec(v), false)),
                    c.lt.map(|v| (v, dec(v), true)),
                    c.negative.then(|| (0, zero(), true)),
                    c.non_positive.then(|| (0, zero(), false)),
                ]
                .into_iter()
                .flatten()
                .collect(),
                false,
            );
            if let Some((_, v, excl)) = upper {
                want[if excl { "x-exclusiveMaxDecimal" } else { "x-maxDecimal" }] = json!(v);
            }
            if let Some(m) = c.multiple_of {
                want["x-multipleOfDecimal"] = json!(dec(m));
            }
            prop_assert_eq!(s.json_schema(), want);
        }
    }
}

// ---------------------------------------------------------------- duration, ports

/// A duration input and the milliseconds it means (`None`: a negative number).
#[derive(Debug, Clone)]
enum DurIn {
    Secs(u64),
    /// Quarter seconds as a JSON float (`0.0`, `-0.25`, `1.5`, …).
    Float(i32),
    Millis(u64),
}

fn dur_in() -> impl Strategy<Value = DurIn> {
    prop_oneof![
        (0u64..6).prop_map(DurIn::Secs),
        (-4i32..24).prop_map(DurIn::Float),
        (0u64..24).prop_map(|q| DurIn::Millis(q * 250)),
    ]
}

proptest! {
    /// Given `duration()` with random inclusive `min` / `max` (multiples of 250 ms)
    /// When whole seconds, float seconds (zero and negatives included) or `"…ms"` arrive
    /// Then a negative number is `too_small` with minimum 0; otherwise the duration is returned,
    /// or below `min` is `too_small` and above `max` is `too_big`, each naming its bound in
    /// seconds.
    #[test]
    fn duration_model(
        min in prop::option::of(0u64..24),
        max in prop::option::of(0u64..24),
        input in dur_in(),
    ) {
        use std::time::Duration;
        let ms = |q: u64| Duration::from_millis(q * 250);
        let mut s = vld::duration();
        if let Some(q) = min { s = s.min(ms(q)); }
        if let Some(q) = max { s = s.max(ms(q)); }
        let (value, millis) = match input {
            DurIn::Secs(n) => (json!(n), Some(n * 1000)),
            DurIn::Float(q) => (json!(f64::from(q) / 4.0), u64::try_from(q).ok().map(|q| q * 250)),
            DurIn::Millis(m) => (json!(format!("{m}ms")), Some(m)),
        };
        let want = match millis {
            None => Some(IssueCode::TooSmall { origin: SizeOrigin::Duration, minimum: json!(0), inclusive: true }),
            Some(m) => {
                let d = Duration::from_millis(m);
                [
                    min.map(ms).filter(|&b| d < b).map(|b| IssueCode::TooSmall {
                        origin: SizeOrigin::Duration, minimum: json!(b.as_secs_f64()), inclusive: true,
                    }),
                    max.map(ms).filter(|&b| d > b).map(|b| IssueCode::TooBig {
                        origin: SizeOrigin::Duration, maximum: json!(b.as_secs_f64()), inclusive: true,
                    }),
                ]
                .into_iter()
                .flatten()
                .next()
            }
        };
        match (s.parse_value(&value), want) {
            (Ok(d), None) => prop_assert_eq!(Some(d), millis.map(Duration::from_millis)),
            (Err(err), Some(code)) => {
                prop_assert_eq!(err.issues.len(), 1);
                prop_assert_eq!(&err.issues[0].code, &code);
            }
            (got, want) => prop_assert!(false, "{value}: got {got:?}, want {want:?}"),
        }
    }

    /// Given `socket_addr()` with random inclusive `min_port` / `max_port`
    /// When IPv4 and IPv6 addresses with ports at and around the bounds arrive
    /// Then the address is returned inside the range; below is `too_small`, above `too_big`, each
    /// with a `port` origin and the bound.
    #[test]
    fn socket_port_model(
        min in prop::option::of(0u16..12),
        max in prop::option::of(0u16..12),
        port in 0u16..12,
        v6 in any::<bool>(),
    ) {
        let mut s = vld::socket_addr();
        if let Some(p) = min { s = s.min_port(p); }
        if let Some(p) = max { s = s.max_port(p); }
        let text = if v6 { format!("[::1]:{port}") } else { format!("127.0.0.1:{port}") };
        let want = [
            min.filter(|&b| port < b).map(|b| IssueCode::TooSmall { origin: SizeOrigin::Port, minimum: b.into(), inclusive: true }),
            max.filter(|&b| port > b).map(|b| IssueCode::TooBig { origin: SizeOrigin::Port, maximum: b.into(), inclusive: true }),
        ]
        .into_iter()
        .flatten()
        .next();
        match (s.parse_value(&json!(text)), want) {
            (Ok(addr), None) => prop_assert_eq!(addr.to_string(), text),
            (Err(err), Some(code)) => prop_assert_eq!(&err.issues[0].code, &code),
            (got, want) => prop_assert!(false, "{text}: got {got:?}, want {want:?}"),
        }
    }
}

// ---------------------------------------------------------------- JSON values

fn arb_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        (-3i64..=3).prop_map(|i| json!(i)),
        (-6i32..=6).prop_map(|h| json!(f64::from(h) / 2.0)),
        prop_oneof![Just("a"), Just("b")].prop_map(|s| json!(s)),
    ];
    leaf.prop_recursive(3, 16, 3, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..3).prop_map(Value::Array),
            prop::collection::btree_map(prop_oneof![Just("a"), Just("b"), Just("c")], inner, 0..3)
                .prop_map(|m| Value::Object(
                    m.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
                )),
        ]
    })
}

/// The same value with every number spelled the other way: integers as floats, integral floats
/// as integers.
fn respell(v: &Value) -> Value {
    match v {
        Value::Number(n) if n.is_f64() => {
            let f = n.as_f64().unwrap();
            if f.fract() == 0.0 {
                json!(f as i64)
            } else {
                v.clone()
            }
        }
        Value::Number(n) => json!(n.as_i64().unwrap() as f64),
        Value::Array(a) => Value::Array(a.iter().map(respell).collect()),
        Value::Object(m) => Value::Object(m.iter().map(|(k, v)| (k.clone(), respell(v))).collect()),
        _ => v.clone(),
    }
}

/// The same container with one more element / key.
fn grow(v: &Value) -> Value {
    let mut v = v.clone();
    match &mut v {
        Value::Array(a) => a.push(Value::Null),
        Value::Object(m) => {
            m.insert("z".into(), Value::Null);
        }
        _ => {}
    }
    v
}

/// JSON equality with numbers compared by value.
fn model_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => model_num(x) == model_num(y),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(a, b)| model_eq(a, b))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| model_eq(v, w)))
        }
        _ => a == b,
    }
}

/// A JSON number as its exact value: an integer, or a float that is not a
/// whole number below 2^127 (those are integers too).
#[derive(PartialEq)]
enum ModelNum {
    Int(i128),
    Frac(f64),
}

fn model_num(n: &serde_json::Number) -> ModelNum {
    if let Some(i) = n.as_i64() {
        return ModelNum::Int(i.into());
    }
    if let Some(u) = n.as_u64() {
        return ModelNum::Int(u.into());
    }
    let f = n.as_f64().unwrap();
    if f.fract() == 0.0 && f.abs() < 2f64.powi(127) {
        ModelNum::Int(f as i128)
    } else {
        ModelNum::Frac(f)
    }
}

fn model_depth(v: &Value) -> usize {
    match v {
        Value::Array(a) => 1 + a.iter().map(model_depth).max().unwrap_or(0),
        Value::Object(m) => 1 + m.values().map(model_depth).max().unwrap_or(0),
        _ => 1,
    }
}

/// `(element, needle)`: the needle is the element respelled, the element plus one element / key,
/// or an unrelated value.
fn json_pair() -> impl Strategy<Value = (Value, Value)> {
    (arb_json(), arb_json(), 0u8..3).prop_map(|(a, other, how)| {
        let needle = match how {
            0 => respell(&a),
            1 => grow(&a),
            _ => other,
        };
        (a, needle)
    })
}

/// Given `contains` and `literal` with integers beyond 2^53
/// When the nearest `f64` spelling of a neighbouring integer arrives
/// Then it is a different value (an `f64` comparison would call them equal),
/// while the same integer written as a whole float matches.
#[test]
fn numbers_compare_exactly_beyond_f64_precision() {
    let big = json!(9_007_199_254_740_993_u64); // 2^53 + 1
    let near = json!(9_007_199_254_740_992.0_f64); // 2^53, its nearest f64
    let contains = vld::array(vld::any()).contains(big.clone());
    assert!(contains.parse_value(&json!([near])).is_err());
    assert!(contains.parse_value(&json!([big.clone()])).is_ok());
    assert!(
        vld::array(vld::any())
            .contains(json!(4_611_686_018_427_387_904_u64)) // 2^62
            .parse_value(&json!([4_611_686_018_427_387_904.0_f64]))
            .is_ok()
    );
    assert!(
        vld::literal(9_007_199_254_740_993_i64)
            .parse_value(&near)
            .is_err()
    );
}

#[derive(Debug, Clone, Copy)]
enum Shape {
    Any,
    Object,
    Array,
}

proptest! {
    /// Given `array(any()).contains(b)` for a random JSON value `b`
    /// When the array `[a]` arrives, where `b` is `a` respelled (`1` ↔ `1.0`), `a` plus one
    /// element / key, or an unrelated value
    /// Then the array is accepted exactly when `a` equals `b` with numbers compared by value
    /// (deep, in arrays and objects, lengths and keys included).
    #[test]
    fn contains_compares_json_by_value((elem, needle) in json_pair()) {
        let ok = vld::array(vld::any())
            .contains(needle.clone())
            .parse_value(&json!([elem.clone()]))
            .is_ok();
        prop_assert_eq!(ok, model_eq(&elem, &needle), "{} in [{}]", needle, elem);
    }

    /// Given `json_value()` as any value, an object or an array, with random required keys and a
    /// random `max_depth` (from 1: every value is at least 1 deep)
    /// When a random JSON tree arrives
    /// Then the wrong container is `invalid_type` (expected `object` / `array`), required keys
    /// demand an object and a missing one is `missing_field` at its key, a tree deeper than
    /// `max_depth` (arrays and objects add a level, scalars are 1) is `json_too_deep`, else the
    /// value comes back unchanged; with `openapi`, the schema is required exactly when `null` is
    /// rejected.
    #[test]
    fn json_value_model(
        shape in prop_oneof![Just(Shape::Any), Just(Shape::Object), Just(Shape::Array)],
        keys in prop::sample::subsequence(vec!["a", "b"], 0..=2),
        max_depth in prop::option::of(1usize..5),
        v in arb_json(),
    ) {
        let mut s = vld::json_value();
        s = match shape {
            Shape::Any => s,
            Shape::Object => s.object(),
            Shape::Array => s.array(),
        };
        for k in &keys { s = s.require_key(*k); }
        if let Some(d) = max_depth { s = s.max_depth(d); }

        // `Err(expected)`: one `invalid_type`; `Ok(issue)`: that `(path, code)`.
        let want: Option<Result<(String, &str), &str>> = match shape {
            Shape::Object if !v.is_object() => Some(Err("object")),
            Shape::Array if !v.is_array() => Some(Err("array")),
            _ if !keys.is_empty() && !v.is_object() => Some(Err("object")),
            _ => keys
                .iter()
                .find(|k| !v.as_object().unwrap().contains_key(**k))
                .map(|k| Ok((format!(".{k}"), "missing_field")))
                .or_else(|| {
                    max_depth
                        .filter(|&d| model_depth(&v) > d)
                        .map(|_| Ok((String::new(), "json_too_deep")))
                }),
        };
        match (s.parse_value(&v), want) {
            (Ok(out), None) => prop_assert_eq!(out, v),
            (Err(err), Some(Err(expected))) => {
                prop_assert_eq!(issues(&err), vec![(String::new(), "invalid_type".to_string())]);
                let is_expected = matches!(&err.issues[0].code, IssueCode::InvalidType { expected: e, .. } if e == expected);
                prop_assert!(is_expected, "{:?}", err.issues[0].code);
            }
            (Err(err), Some(Ok((path, code)))) => {
                prop_assert_eq!(issues(&err), vec![(path, code.to_string())]);
                if code == "json_too_deep" {
                    prop_assert_eq!(&err.issues[0].code, &IssueCode::JsonTooDeep { max_depth: max_depth.unwrap() });
                }
            }
            (got, want) => prop_assert!(false, "{v}: got {got:?}, want {want:?}"),
        }

        #[cfg(feature = "openapi")]
        {
            use vld::openapi::JsonSchema;
            prop_assert_eq!(s.is_required(), s.parse_value(&Value::Null).is_err());
        }
    }
}

// ---------------------------------------------------------------- literals

proptest! {
    /// Given a literal of every kind (`&'static str`, `String`, `i64`, `f64`, `f32`, `bool`)
    /// When its own JSON form arrives (an `i64` also as an integral float), and a different value
    /// Then the literal returns its own value; the different value is `invalid_literal`; the
    /// `IntoLiteral` conversions give the JSON value and the message form (strings quoted,
    /// numbers and booleans as Rust prints them).
    #[test]
    fn literal_model(
        text in "[a-z]{0,4}",
        other in "[a-z]{0,4}",
        int in any::<i64>(),
        small in -(1i64 << 52)..(1i64 << 52),
        float in -1e6f64..1e6,
        single in -1e3f32..1e3,
        flag in any::<bool>(),
    ) {
        use vld::primitives::IntoLiteral;
        let differs = |s: &str| if s == text { format!("{s}x") } else { s.to_string() };

        let leaked: &'static str = Box::leak(text.clone().into_boxed_str());
        prop_assert_eq!(vld::literal(leaked).parse_value(&json!(text)).unwrap(), text.clone());
        prop_assert_eq!(leaked.display(), format!("\"{text}\""));
        prop_assert_eq!(IntoLiteral::to_json_value(&leaked), json!(text));

        let owned = vld::literal(text.clone());
        prop_assert_eq!(owned.expected_value(), &json!(text));
        prop_assert_eq!(owned.parse_value(&json!(text)).unwrap(), text.clone());
        prop_assert!(owned.parse_value(&json!(differs(&other))).is_err());
        prop_assert_eq!(text.display(), format!("\"{text}\""));
        prop_assert_eq!(String::extract(&json!(text)), Some(text.clone()));

        prop_assert_eq!(vld::literal(int).parse_value(&json!(int)).unwrap(), int);
        prop_assert!(vld::literal(int).parse_value(&json!(int.wrapping_add(1))).is_err());
        prop_assert_eq!(vld::literal(small).parse_value(&json!(small as f64)).unwrap(), small);
        prop_assert_eq!(int.display(), int.to_string());

        prop_assert_eq!(vld::literal(float).parse_value(&json!(float)).unwrap(), float);
        prop_assert_eq!(float.display(), float.to_string());
        prop_assert_eq!(f64::extract(&json!(float)), Some(float));
        prop_assert_eq!(single.display(), single.to_string());
        prop_assert_eq!(vld::literal(single).parse_value(&IntoLiteral::to_json_value(&single)).unwrap(), single);

        prop_assert_eq!(vld::literal(flag).parse_value(&json!(flag)).unwrap(), flag);
        prop_assert!(vld::literal(flag).parse_value(&json!(!flag)).is_err());
        prop_assert_eq!(flag.display(), flag.to_string());
        prop_assert_eq!(bool::extract(&json!(flag)), Some(flag));
    }
}
