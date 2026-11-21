//! Model test: random schema trees fed random inputs.
//!
//! proptest builds a schema tree (strings, integers, `u8`, booleans, literals,
//! enums, arrays, records, objects in every unknown-key mode with `partial`,
//! `catchall` and key-count limits, discriminated unions, intersections,
//! `optional` / `nullable` / `nullish`, `with_default`, `blank_as`, `catch`,
//! `refine`, `transform`, `describe`, unions; with `min` / `max`, `trim`,
//! `coerce`, `clamp`) and an input shaped like it (valid values, wrong
//! types, `null`, blank and numeric strings, missing and extra keys, huge and
//! fractional numbers). A small model of vld's documented rules predicts the
//! result: the output JSON on success, the exact `(path, code)` issue list on
//! failure. For trees without lossy steps the JSON Schema vld emits must give
//! the same verdict (draft 2020-12, `jsonschema`).
//!
//! On failure proptest shrinks to the smallest tree and input that disagree.

use crate::common::*;

use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use serde_json::{Map, Value, json};
use vld::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mode {
    Strip,
    Strict,
    Passthrough,
}

#[derive(Debug, Clone)]
enum Node {
    Str {
        min: Option<usize>,
        max: Option<usize>,
        trim: bool,
        coerce: bool,
    },
    Int {
        min: Option<i64>,
        max: Option<i64>,
        clamp: bool,
        coerce: bool,
    },
    U8 {
        clamp: bool,
    },
    Bool {
        coerce: bool,
    },
    Lit(&'static str),
    Enum(Vec<&'static str>),
    Array {
        item: Box<Node>,
        min: Option<usize>,
        max: Option<usize>,
    },
    Object {
        fields: Vec<(&'static str, Node)>,
        mode: Mode,
        partial: bool,
        catchall: Option<Box<Node>>,
        min_keys: Option<usize>,
        max_keys: Option<usize>,
    },
    Record {
        value: Box<Node>,
        min: Option<usize>,
        max: Option<usize>,
    },
    /// `discriminated_union("type")`: `(tag, fields)`; each variant object
    /// has a required `"type": literal(tag)` field first.
    Disc(Vec<(&'static str, Vec<(&'static str, Node)>)>),
    Intersection(Box<Node>, Box<Node>),
    /// `refine(not empty)`: strings, arrays and objects must be non-empty, `null` fails.
    Refine(Box<Node>),
    /// `transform(|v| {"t": v})`.
    Transform(Box<Node>),
    Describe(Box<Node>),
    Optional(Box<Node>),
    Nullable(Box<Node>),
    Nullish(Box<Node>),
    Default(Box<Node>),
    BlankAs(Box<Node>),
    Catch(Box<Node>),
    Union(Box<Node>, Box<Node>),
}

/// `Err` = the `(path, code)` issues.
type Outcome = Result<Value, Vec<(String, String)>>;

const KEYS: [&str; 4] = ["a", "b", "c", "d"];
const WORDS: [&str; 3] = ["x", "yes", "on"];

// ---------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------

fn leaf() -> impl Strategy<Value = Node> {
    let len = || proptest::option::of(0usize..4);
    let bound = || proptest::option::of(-3i64..300);
    prop_oneof![
        (len(), len(), any::<bool>(), any::<bool>()).prop_map(|(min, max, trim, coerce)| {
            Node::Str {
                min,
                max,
                trim,
                coerce,
            }
        }),
        (bound(), bound(), any::<bool>(), any::<bool>()).prop_map(|(min, max, clamp, coerce)| {
            Node::Int {
                min,
                max,
                clamp,
                coerce,
            }
        }),
        any::<bool>().prop_map(|clamp| Node::U8 { clamp }),
        any::<bool>().prop_map(|coerce| Node::Bool { coerce }),
        proptest::sample::select(&WORDS[..]).prop_map(Node::Lit),
        proptest::sample::subsequence(&WORDS[..], 1..=3).prop_map(Node::Enum),
    ]
}

fn node() -> impl Strategy<Value = Node> {
    leaf().prop_recursive(3, 24, 4, |inner| {
        let boxed = inner.clone();
        let b = move || boxed.clone().prop_map(Box::new);
        prop_oneof![
            (
                b(),
                proptest::option::of(0usize..3),
                proptest::option::of(0usize..3)
            )
                .prop_map(|(item, min, max)| Node::Array { item, min, max }),
            (
                proptest::sample::subsequence(&KEYS[..], 0..=3),
                proptest::collection::vec(inner.clone(), 3),
                prop_oneof![
                    Just(Mode::Strip),
                    Just(Mode::Strict),
                    Just(Mode::Passthrough)
                ],
                proptest::option::weighted(0.2, b()),
                proptest::bool::weighted(0.2),
                proptest::option::weighted(0.2, 0usize..4),
                proptest::option::weighted(0.2, 0usize..4),
            )
                .prop_map(
                    |(keys, nodes, mode, catchall, partial, min_keys, max_keys)| {
                        Node::Object {
                            fields: keys.into_iter().zip(nodes).collect(),
                            mode,
                            partial,
                            catchall,
                            min_keys,
                            max_keys,
                        }
                    }
                ),
            (
                b(),
                proptest::option::of(0usize..3),
                proptest::option::of(0usize..3)
            )
                .prop_map(|(value, min, max)| Node::Record { value, min, max }),
            proptest::collection::vec(
                (
                    proptest::sample::subsequence(&KEYS[..], 0..=2),
                    proptest::collection::vec(inner.clone(), 2),
                ),
                1..=3,
            )
            .prop_map(|vs| Node::Disc(
                vs.into_iter()
                    .zip(WORDS)
                    .map(|((keys, nodes), tag)| (tag, keys.into_iter().zip(nodes).collect()))
                    .collect()
            )),
            (b(), b()).prop_map(|(x, y)| Node::Intersection(x, y)),
            b().prop_map(Node::Refine),
            b().prop_map(Node::Transform),
            b().prop_map(Node::Describe),
            b().prop_map(Node::Optional),
            b().prop_map(Node::Nullable),
            b().prop_map(Node::Nullish),
            b().prop_map(Node::Default),
            b().prop_map(Node::BlankAs),
            b().prop_map(Node::Catch),
            (b(), b()).prop_map(|(x, y)| Node::Union(x, y)),
        ]
    })
}

/// Any small JSON value: the "noise" mixed into every input.
fn noise() -> BoxedStrategy<Value> {
    prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::from),
        (-5i64..400).prop_map(Value::from),
        prop_oneof![Just(1.5f64), Just(-0.5), Just(2.0), Just(1e20), Just(-1e20)]
            .prop_map(Value::from),
        Just(json!(u64::MAX)),
        prop_oneof![
            Just(""),
            Just("  "),
            Just(" ab "),
            Just("abcd"),
            Just("12"),
            Just("-4"),
            Just("2.0"),
            Just("2.5"),
            Just("1e3"),
            Just("true"),
            Just("0"),
            Just("1"),
            Just("x"),
            Just("yes"),
            Just("on"),
            Just("é\u{3000}"),
        ]
        .prop_map(Value::from),
        Just(json!([])),
        Just(json!({})),
    ]
    .boxed()
}

/// An input shaped like `n`, with noise at every level.
fn input(n: &Node) -> BoxedStrategy<Value> {
    let shaped: BoxedStrategy<Value> = match n {
        Node::Array { item, .. } => proptest::collection::vec(input(item), 0..4)
            .prop_map(Value::Array)
            .boxed(),
        Node::Object { fields, .. } => object_input(fields, None),
        Node::Record { value, .. } => proptest::collection::btree_map("[a-e]", input(value), 0..4)
            .prop_map(|m| Value::Object(m.into_iter().collect()))
            .boxed(),
        Node::Disc(vs) => {
            let options: Vec<BoxedStrategy<Value>> = vs
                .iter()
                .map(|(tag, fields)| object_input(fields, Some(tag)))
                .collect();
            proptest::strategy::Union::new(options).boxed()
        }
        Node::Intersection(a, b) => prop_oneof![input(a), input(b)].boxed(),
        Node::Refine(i) | Node::Transform(i) | Node::Describe(i) => input(i),
        Node::Optional(i)
        | Node::Nullable(i)
        | Node::Nullish(i)
        | Node::Default(i)
        | Node::BlankAs(i)
        | Node::Catch(i) => input(i),
        Node::Union(a, b) => prop_oneof![input(a), input(b)].boxed(),
        Node::Lit(w) => Just(Value::from(*w)).boxed(),
        Node::Enum(ws) => proptest::sample::select(ws.clone())
            .prop_map(Value::from)
            .boxed(),
        _ => noise(),
    };
    prop_oneof![4 => shaped, 1 => noise()].boxed()
}

/// An object input for `fields`, with `"type": tag` for a discriminated-union variant.
fn object_input(
    fields: &[(&'static str, Node)],
    tag: Option<&'static str>,
) -> BoxedStrategy<Value> {
    let parts: Vec<BoxedStrategy<Option<(String, Value)>>> = fields
        .iter()
        .map(|(k, f)| {
            let k = k.to_string();
            proptest::option::weighted(0.8, input(f))
                .prop_map(move |v| v.map(|v| (k.clone(), v)))
                .boxed()
        })
        .collect();
    let extra = proptest::option::weighted(0.3, ("[a-e]", noise()));
    let tag = proptest::option::weighted(0.9, Just(tag)).prop_map(|t| t.flatten());
    (parts, extra, tag)
        .prop_map(|(parts, extra, tag)| {
            let mut m: Map<String, Value> = parts.into_iter().flatten().collect();
            if let Some((k, v)) = extra {
                m.entry(k).or_insert(v);
            }
            if let Some(t) = tag {
                m.insert("type".into(), Value::from(t));
            }
            Value::Object(m)
        })
        .boxed()
}

// ---------------------------------------------------------------------------
// The schema under test
// ---------------------------------------------------------------------------

fn build(n: &Node) -> Erased {
    match n {
        Node::Str {
            min,
            max,
            trim,
            coerce,
        } => {
            let mut s = vld::string();
            if *trim {
                s = s.trim();
            }
            if *coerce {
                s = s.coerce();
            }
            if let Some(m) = min {
                s = s.min(*m);
            }
            if let Some(m) = max {
                s = s.max(*m);
            }
            Erased::new(s)
        }
        Node::Int {
            min,
            max,
            clamp,
            coerce,
        } => {
            let mut s = vld::number().int();
            if let Some(m) = min {
                s = s.min(*m);
            }
            if let Some(m) = max {
                s = s.max(*m);
            }
            if *clamp {
                s = s.clamp();
            }
            if *coerce {
                s = s.coerce();
            }
            Erased::new(s)
        }
        Node::U8 { clamp } => Erased::new(if *clamp { vld::u8().clamp() } else { vld::u8() }),
        Node::Bool { coerce } => Erased::new(if *coerce {
            vld::boolean().coerce()
        } else {
            vld::boolean()
        }),
        Node::Lit(w) => Erased::new(vld::literal(*w)),
        Node::Enum(ws) => Erased::new(vld::enumeration(ws)),
        Node::Array { item, min, max } => {
            let mut s = vld::array(build(item));
            if let Some(m) = min {
                s = s.min_len(*m);
            }
            if let Some(m) = max {
                s = s.max_len(*m);
            }
            Erased::new(s)
        }
        Node::Object {
            fields,
            mode,
            partial,
            catchall,
            min_keys,
            max_keys,
        } => {
            let mut o = vld::object();
            for (k, f) in fields {
                o = o.field(*k, build(f));
            }
            o = match mode {
                Mode::Strip => o.strip(),
                Mode::Strict => o.strict(),
                Mode::Passthrough => o.passthrough(),
            };
            if *partial {
                o = o.partial();
            }
            if let Some(c) = catchall {
                o = o.catchall(build(c));
            }
            if let Some(n) = min_keys {
                o = o.min_keys(*n);
            }
            if let Some(n) = max_keys {
                o = o.max_keys(*n);
            }
            Erased::new(o)
        }
        Node::Record { value, min, max } => {
            let mut r = vld::record(build(value));
            if let Some(n) = min {
                r = r.min_keys(*n);
            }
            if let Some(n) = max {
                r = r.max_keys(*n);
            }
            Erased::new(r)
        }
        Node::Disc(vs) => {
            let mut d = vld::discriminated_union("type");
            for (tag, fields) in vs {
                let mut o = vld::object().field("type", vld::literal(*tag));
                for (k, f) in fields {
                    o = o.field(*k, build(f));
                }
                d = d.variant_str(tag, o);
            }
            Erased::new(d)
        }
        Node::Intersection(a, b) => Erased::new(vld::intersection(build(a), build(b))),
        Node::Refine(i) => Erased::new(build(i).refine(not_empty, "must not be empty")),
        Node::Transform(i) => Erased::new(build(i).transform(|v| json!({ "t": v }))),
        Node::Describe(i) => Erased::new(build(i).describe("described")),
        Node::Optional(i) => Erased::new(build(i).optional()),
        Node::Nullable(i) => Erased::new(build(i).nullable()),
        Node::Nullish(i) => Erased::new(build(i).nullish()),
        Node::Default(i) => Erased::new(build(i).with_default(fallback(i))),
        Node::BlankAs(i) => Erased::new(build(i).blank_as(fallback(i))),
        Node::Catch(i) => Erased::new(build(i).catch(fallback(i))),
        Node::Union(a, b) => Erased::new(vld::union(build(a), build(b))),
    }
}

fn not_empty(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
        _ => true,
    }
}

/// The value `with_default` / `blank_as` / `catch` fall back to.
fn fallback(n: &Node) -> Value {
    json!({ "fallback": format!("{n:?}").len() })
}

// ---------------------------------------------------------------------------
// The model
// ---------------------------------------------------------------------------

fn fail(code: &str) -> Outcome {
    Err(vec![(String::new(), code.to_string())])
}

fn prefix(issues: Vec<(String, String)>, seg: &str) -> Vec<(String, String)> {
    issues
        .into_iter()
        .map(|(p, c)| (format!("{seg}{p}"), c))
        .collect()
}

/// An integer as vld reads it (`None` = not a number at all).
enum IntIn {
    Int(i128),
    NotInt,
    Type,
}

fn read_int(v: &Value, coerce: bool) -> IntIn {
    let from_f64 = |f: f64| {
        if f.fract() != 0.0 {
            IntIn::NotInt
        } else {
            IntIn::Int(f as i128)
        }
    };
    match v {
        Value::Number(n) => match (n.as_i64(), n.as_u64()) {
            (Some(i), _) => IntIn::Int(i.into()),
            (_, Some(u)) => IntIn::Int(u.into()),
            _ => from_f64(n.as_f64().unwrap()),
        },
        Value::String(s) if coerce => match s.parse::<i128>() {
            Ok(i) => IntIn::Int(i),
            Err(_) => match s.parse::<f64>() {
                Ok(f) if f.is_finite() => from_f64(f),
                _ => IntIn::Type,
            },
        },
        Value::Bool(b) if coerce => IntIn::Int(i128::from(*b)),
        _ => IntIn::Type,
    }
}

/// Bound checks of an integer (`min` then `max`, as built), then the type range.
fn int_checks(n: i128, min: Option<i64>, max: Option<i64>, range: (i128, i128)) -> Outcome {
    let mut out = Vec::new();
    if min.is_some_and(|m| n < m.into()) {
        out.push((String::new(), "too_small".into()));
    }
    if max.is_some_and(|m| n > m.into()) {
        out.push((String::new(), "too_big".into()));
    }
    if !out.is_empty() {
        return Err(out);
    }
    if n < range.0 {
        return fail("too_small");
    }
    if n > range.1 {
        return fail("too_big");
    }
    Ok(json!(n as i64))
}

fn clamp(mut n: i128, min: Option<i64>, max: Option<i64>, range: (i128, i128)) -> i128 {
    if let Some(m) = min.map(i128::from).filter(|m| n < *m) {
        n = m;
    }
    if let Some(m) = max.map(i128::from).filter(|m| n > *m) {
        n = m;
    }
    n.clamp(range.0, range.1)
}

fn is_blank(v: &Value) -> bool {
    v.as_str().is_some_and(|s| s.trim().is_empty())
}

fn predict(n: &Node, v: &Value) -> Outcome {
    match n {
        Node::Str {
            min,
            max,
            trim,
            coerce,
        } => {
            let mut s = match v {
                Value::String(s) => s.clone(),
                Value::Number(x) if *coerce => x.to_string(),
                Value::Bool(b) if *coerce => b.to_string(),
                _ => return fail("invalid_type"),
            };
            if *trim {
                s = s.trim().to_string();
            }
            let len = s.chars().count();
            let mut out = Vec::new();
            if min.is_some_and(|m| len < m) {
                out.push((String::new(), "too_small".into()));
            }
            if max.is_some_and(|m| len > m) {
                out.push((String::new(), "too_big".into()));
            }
            if out.is_empty() {
                Ok(json!(s))
            } else {
                Err(out)
            }
        }
        Node::Int {
            min,
            max,
            clamp: c,
            coerce,
        } => {
            let range = (i64::MIN.into(), i64::MAX.into());
            match read_int(v, *coerce) {
                IntIn::Type => fail("invalid_type"),
                IntIn::NotInt => fail("not_int"),
                IntIn::Int(i) => {
                    let i = if *c { clamp(i, *min, *max, range) } else { i };
                    int_checks(i, *min, *max, range)
                }
            }
        }
        Node::U8 { clamp: c } => match read_int(v, false) {
            IntIn::Type => fail("invalid_type"),
            IntIn::NotInt => fail("not_int"),
            IntIn::Int(i) => {
                let i = if *c { i.clamp(0, 255) } else { i };
                int_checks(i, None, None, (0, 255))
            }
        },
        Node::Bool { coerce } => match v {
            Value::Bool(b) => Ok(json!(b)),
            Value::String(s) if *coerce => match s.as_str() {
                "true" | "1" => Ok(json!(true)),
                "false" | "0" => Ok(json!(false)),
                _ => fail("invalid_type"),
            },
            Value::Number(x) if *coerce => Ok(json!(x.as_f64() != Some(0.0))),
            _ => fail("invalid_type"),
        },
        Node::Lit(w) => {
            if v == &json!(w) {
                Ok(v.clone())
            } else {
                fail("invalid_literal")
            }
        }
        Node::Enum(ws) => match v.as_str() {
            Some(s) if ws.contains(&s) => Ok(v.clone()),
            Some(_) => fail("invalid_enum_value"),
            None => fail("invalid_type"),
        },
        Node::Array { item, min, max } => {
            let Some(items) = v.as_array() else {
                return fail("invalid_type");
            };
            let mut errs = Vec::new();
            if min.is_some_and(|m| items.len() < m) {
                errs.push((String::new(), "too_small".into()));
            }
            if max.is_some_and(|m| items.len() > m) {
                errs.push((String::new(), "too_big".into()));
            }
            let mut out = Vec::new();
            for (i, x) in items.iter().enumerate() {
                match predict(item, x) {
                    Ok(o) => out.push(o),
                    Err(e) => errs.extend(prefix(e, &format!("[{i}]"))),
                }
            }
            if errs.is_empty() {
                Ok(Value::Array(out))
            } else {
                Err(errs)
            }
        }
        Node::Object {
            fields,
            mode,
            partial,
            catchall,
            min_keys,
            max_keys,
        } => {
            let Some(obj) = v.as_object() else {
                return fail("invalid_type");
            };
            let mut errs = Vec::new();
            if min_keys.is_some_and(|m| obj.len() < m) {
                errs.push((String::new(), "too_small".into()));
            }
            if max_keys.is_some_and(|m| obj.len() > m) {
                errs.push((String::new(), "too_big".into()));
            }
            let mut out = Map::new();
            for (k, f) in fields {
                let r = match obj.get(*k) {
                    // `partial()`: `null` and a missing key are `null` unless
                    // the field itself takes them (a default, …).
                    Some(Value::Null) if *partial => {
                        Ok(predict(f, &Value::Null).unwrap_or(Value::Null))
                    }
                    None if *partial => Ok(raw_missing(f).unwrap_or(Value::Null)),
                    Some(x) => predict(f, x),
                    None => predict_missing(f),
                };
                match r {
                    Ok(o) => {
                        out.insert(k.to_string(), o);
                    }
                    Err(e) => errs.extend(prefix(e, &format!(".{k}"))),
                }
            }
            for (k, x) in obj
                .iter()
                .filter(|(k, _)| !fields.iter().any(|(f, _)| f == k))
            {
                if let Some(c) = catchall {
                    match predict(c, x) {
                        Ok(o) => {
                            out.insert(k.clone(), o);
                        }
                        Err(e) => errs.extend(prefix(e, &format!(".{k}"))),
                    }
                    continue;
                }
                match mode {
                    Mode::Strip => {}
                    Mode::Strict => errs.push((format!(".{k}"), "unrecognized_field".into())),
                    Mode::Passthrough => {
                        out.insert(k.clone(), x.clone());
                    }
                }
            }
            if errs.is_empty() {
                Ok(Value::Object(out))
            } else {
                Err(errs)
            }
        }
        Node::Record { value, min, max } => {
            let Some(obj) = v.as_object() else {
                return fail("invalid_type");
            };
            let mut errs = Vec::new();
            if min.is_some_and(|m| obj.len() < m) {
                errs.push((String::new(), "too_small".into()));
            }
            if max.is_some_and(|m| obj.len() > m) {
                errs.push((String::new(), "too_big".into()));
            }
            let mut out = Map::new();
            for (k, x) in obj {
                match predict(value, x) {
                    Ok(o) => {
                        out.insert(k.clone(), o);
                    }
                    Err(e) => errs.extend(prefix(e, &format!(".{k}"))),
                }
            }
            if errs.is_empty() {
                Ok(Value::Object(out))
            } else {
                Err(errs)
            }
        }
        Node::Disc(vs) => {
            let Some(obj) = v.as_object() else {
                return fail("invalid_type");
            };
            let Some(tag) = obj.get("type") else {
                return Err(vec![(".type".into(), "missing_field".into())]);
            };
            match vs.iter().find(|(t, _)| tag == &json!(t)) {
                Some((t, fields)) => predict(&variant_object(t, fields), v),
                None => Err(vec![(".type".into(), "invalid_discriminator".into())]),
            }
        }
        Node::Intersection(a, b) => match (predict(a, v), predict(b, v)) {
            (Ok(o), Ok(_)) => Ok(o),
            (ra, rb) => Err([ra.err(), rb.err()]
                .into_iter()
                .flatten()
                .flatten()
                .collect()),
        },
        Node::Refine(i) => predict(i, v).and_then(refined),
        Node::Transform(i) => predict(i, v).map(|o| json!({ "t": o })),
        Node::Describe(i) => predict(i, v),
        Node::Optional(i) | Node::Nullable(i) | Node::Nullish(i) => {
            if v.is_null() {
                Ok(Value::Null)
            } else {
                predict(i, v)
            }
        }
        Node::Default(i) => {
            if v.is_null() {
                Ok(fallback(i))
            } else {
                predict(i, v)
            }
        }
        Node::BlankAs(i) => {
            if v.is_null() || is_blank(v) {
                Ok(fallback(i))
            } else {
                predict(i, v)
            }
        }
        Node::Catch(i) => Ok(predict(i, v).unwrap_or_else(|_| fallback(i))),
        Node::Union(a, b) => {
            let ea = match predict(a, v) {
                Ok(o) => return Ok(o),
                Err(e) => e,
            };
            let eb = match predict(b, v) {
                Ok(o) => return Ok(o),
                Err(e) => e,
            };
            let mut out = vec![(String::new(), "invalid_union".to_string())];
            for e in [ea, eb] {
                // A nested union's own `invalid_union` head is flattened away.
                let nested = e
                    .first()
                    .is_some_and(|(p, c)| p.is_empty() && c == "invalid_union");
                out.extend(e.into_iter().skip(usize::from(nested)));
            }
            Err(out)
        }
    }
}

fn refined(o: Value) -> Outcome {
    if not_empty(&o) { Ok(o) } else { fail("custom") }
}

/// The object a discriminated-union variant parses with.
fn variant_object(tag: &'static str, fields: &[(&'static str, Node)]) -> Node {
    let mut all = vec![("type", Node::Lit(tag))];
    all.extend(fields.iter().cloned());
    Node::Object {
        fields: all,
        mode: Mode::Strip,
        partial: false,
        catchall: None,
        min_keys: None,
        max_keys: None,
    }
}

/// `parse_missing` before the object turns a rejected `null` into `missing_field`.
fn raw_missing(n: &Node) -> Outcome {
    match n {
        Node::Optional(_) | Node::Nullish(_) => Ok(Value::Null),
        Node::Nullable(_) => fail("missing_field"),
        Node::Default(i) | Node::BlankAs(i) => Ok(fallback(i)),
        Node::Catch(i) => Ok(raw_missing(i).unwrap_or_else(|_| fallback(i))),
        Node::Refine(i) => raw_missing(i).and_then(refined),
        Node::Transform(i) => raw_missing(i).map(|o| json!({ "t": o })),
        Node::Describe(i) => raw_missing(i),
        _ => predict(n, &Value::Null),
    }
}

/// An absent object key.
fn predict_missing(n: &Node) -> Outcome {
    // A required field that rejects `null` reports `missing_field` alone.
    match raw_missing(n) {
        Err(e) if e.iter().any(|(p, c)| p.is_empty() && c == "invalid_type") => {
            fail("missing_field")
        }
        r => r,
    }
}

/// No lossy step: the JSON Schema verdict must equal vld's.
#[cfg_attr(not(feature = "openapi"), allow(dead_code))]
fn exact(n: &Node) -> bool {
    match n {
        Node::Str { trim, coerce, .. } => !trim && !coerce,
        Node::Int { clamp, coerce, .. } => !clamp && !coerce,
        Node::U8 { clamp } => !clamp,
        Node::Bool { coerce } => !coerce,
        Node::Lit(_) | Node::Enum(_) => true,
        Node::Array { item, .. } => exact(item),
        Node::Object {
            fields,
            partial,
            catchall,
            ..
        } => {
            // `partial()` fields accept `null` that their schema leaves out.
            !partial
                && fields.iter().all(|(_, f)| exact(f))
                && catchall.as_deref().is_none_or(exact)
        }
        Node::Record { value, .. } => exact(value),
        Node::Disc(vs) => vs.iter().all(|(_, fs)| fs.iter().all(|(_, f)| exact(f))),
        Node::Intersection(a, b) => exact(a) && exact(b),
        // A refine predicate has no JSON Schema form.
        Node::Refine(_) => false,
        Node::Transform(i) | Node::Describe(i) => exact(i),
        // `optional` accepts `null` that its schema leaves out; defaults and
        // catch replace values.
        Node::Optional(_) | Node::Default(_) | Node::BlankAs(_) | Node::Catch(_) => false,
        Node::Nullable(i) | Node::Nullish(i) => exact(i),
        Node::Union(a, b) => exact(a) && exact(b),
    }
}

fn check(n: &Node, v: &Value) -> Result<(), TestCaseError> {
    let got = build(n).parse_value(v).map_err(|e| issues(&e));
    let want = predict(n, v);
    prop_assert_eq!(&got, &want, "schema {:?}\ninput {}", n, v);

    // Known: `number().int()` rejects integers beyond `i64` but its schema
    // (`{"type": "integer"}`, no `format`) has no range.
    #[cfg(feature = "openapi")]
    if exact(n) && !beyond_i64(v) {
        use vld::openapi::JsonSchema;
        let schema = build(n).json_schema();
        let validator = jsonschema::draft202012::new(&schema)
            .map_err(|e| TestCaseError::fail(format!("bad schema {schema}: {e}")))?;
        prop_assert_eq!(
            validator.is_valid(v),
            got.is_ok(),
            "JSON Schema {} disagrees on {}",
            schema,
            v
        );
    }
    Ok(())
}

#[cfg_attr(not(feature = "openapi"), allow(dead_code))]
fn beyond_i64(v: &Value) -> bool {
    match v {
        Value::Number(n) => n.as_i64().is_none() && n.as_f64().is_some_and(|f| f.fract() == 0.0),
        Value::Array(a) => a.iter().any(beyond_i64),
        Value::Object(o) => o.values().any(beyond_i64),
        _ => false,
    }
}

#[test]
fn schemas_agree_with_the_model() {
    let mut runner = TestRunner::new(Config {
        cases: 3000,
        ..Config::default()
    });
    let strat = node().prop_flat_map(|n| {
        let i = input(&n);
        (Just(n), i)
    });
    runner.run(&strat, |(n, v)| check(&n, &v)).unwrap();
}
