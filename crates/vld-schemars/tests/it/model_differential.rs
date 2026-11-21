//! Model test: the JSON Schema vld emits, run through `validate_with_schema`,
//! accepts exactly the values vld accepts.
//!
//! proptest builds a random vld schema tree (strings with lengths, prefixes,
//! suffixes and `email`; integers and floats with inclusive / exclusive
//! bounds; booleans, literals, enums; arrays with lengths and `unique`;
//! objects, strict or stripping; `optional` / `nullable` / `nullish`; unions)
//! and an input shaped like it (valid values, wrong types, `null`, missing and
//! extra keys). vld itself is the oracle: `validate_with_schema(json_schema)`
//! must give vld's verdict. The same schema must survive the schemars round
//! trip unchanged and validate the same through `validate_with_schemars`, and
//! `to_openapi3` must be idempotent and leave no 2020-12-only keyword.
//!
//! On failure proptest shrinks to the smallest tree and input that disagree.

use crate::common::{Erased, pairs};

use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use serde_json::{Map, Value, json};
use vld::openapi::JsonSchema;
use vld::prelude::*;

#[derive(Debug, Clone)]
enum Node {
    Str {
        min: Option<usize>,
        max: Option<usize>,
        prefix: Option<&'static str>,
        suffix: Option<&'static str>,
        email: bool,
    },
    Int {
        min: Option<i64>,
        max: Option<i64>,
        gt: Option<i64>,
    },
    Float {
        min: Option<f64>,
        lt: Option<f64>,
    },
    Bool,
    Lit(&'static str),
    Enum(Vec<&'static str>),
    Array {
        item: Box<Node>,
        min: Option<usize>,
        max: Option<usize>,
        unique: bool,
    },
    Object {
        fields: Vec<(&'static str, Node)>,
        strict: bool,
    },
    Optional(Box<Node>),
    Nullable(Box<Node>),
    Nullish(Box<Node>),
    Union(Box<Node>, Box<Node>),
}

const KEYS: [&str; 3] = ["a", "b", "c"];
const STRINGS: [&str; 10] = [
    "", "a", "ab", "abc", "a@b", "x@y.z", "@b", "a b@c", "äöü", "abcdef",
];

fn leaf() -> impl Strategy<Value = Node> {
    let len = || proptest::option::of(0usize..5);
    let lit = || proptest::option::of(proptest::sample::select(vec!["a", "ab", "c"]));
    let int = || proptest::option::of(-3i64..8);
    prop_oneof![
        (len(), len(), lit(), lit(), any::<bool>()).prop_map(
            |(min, max, prefix, suffix, email)| {
                Node::Str {
                    min,
                    max,
                    prefix,
                    suffix,
                    email,
                }
            }
        ),
        (int(), int(), int()).prop_map(|(min, max, gt)| Node::Int { min, max, gt }),
        (
            proptest::option::of(proptest::sample::select(vec![-1.5, 0.0, 2.5])),
            proptest::option::of(proptest::sample::select(vec![0.5, 3.0, 10.0])),
        )
            .prop_map(|(min, lt)| Node::Float { min, lt }),
        Just(Node::Bool),
        proptest::sample::select(vec!["a", "x@y.z"]).prop_map(Node::Lit),
        proptest::sample::subsequence(vec!["a", "ab", "abc", "zz"], 1..4).prop_map(Node::Enum),
    ]
}

fn node() -> impl Strategy<Value = Node> {
    leaf().prop_recursive(3, 24, 3, |inner| {
        let len = || proptest::option::of(0usize..4);
        prop_oneof![
            (inner.clone(), len(), len(), any::<bool>()).prop_map(|(i, min, max, unique)| {
                Node::Array {
                    item: Box::new(i),
                    min,
                    max,
                    unique,
                }
            }),
            (
                proptest::collection::vec(inner.clone(), 0..=3),
                any::<bool>()
            )
                .prop_map(|(fs, strict)| Node::Object {
                    fields: KEYS.iter().copied().zip(fs).collect(),
                    strict,
                }),
            inner.clone().prop_map(|i| Node::Optional(Box::new(i))),
            inner.clone().prop_map(|i| Node::Nullable(Box::new(i))),
            inner.clone().prop_map(|i| Node::Nullish(Box::new(i))),
            (inner.clone(), inner).prop_map(|(a, b)| Node::Union(Box::new(a), Box::new(b))),
        ]
    })
}

fn noise() -> BoxedStrategy<Value> {
    prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::from),
        (-5i64..12).prop_map(Value::from),
        proptest::sample::select(vec![0.5, -1.5, 2.0, 2.5, 9.75]).prop_map(Value::from),
        Just(json!(u64::MAX)),
        proptest::sample::select(STRINGS.to_vec()).prop_map(Value::from),
        Just(json!([])),
        Just(json!({})),
    ]
    .boxed()
}

fn input(n: &Node) -> BoxedStrategy<Value> {
    let shaped = match n {
        Node::Array { item, .. } => proptest::collection::vec(input(item), 0..5)
            .prop_map(Value::Array)
            .boxed(),
        Node::Object { fields, .. } => {
            let parts: Vec<BoxedStrategy<Option<(String, Value)>>> = fields
                .iter()
                .map(|(k, f)| {
                    let k = k.to_string();
                    proptest::option::weighted(0.85, input(f))
                        .prop_map(move |v| v.map(|v| (k.clone(), v)))
                        .boxed()
                })
                .collect();
            (parts, proptest::option::weighted(0.2, noise()))
                .prop_map(|(kvs, extra)| {
                    let mut m: Map<String, Value> = kvs.into_iter().flatten().collect();
                    if let Some(x) = extra {
                        m.insert("zz".into(), x);
                    }
                    Value::Object(m)
                })
                .boxed()
        }
        Node::Optional(i) | Node::Nullable(i) | Node::Nullish(i) => input(i),
        Node::Union(a, b) => prop_oneof![input(a), input(b)].boxed(),
        Node::Lit(w) => Just(Value::from(*w)).boxed(),
        Node::Enum(ws) => proptest::sample::select(ws.clone())
            .prop_map(Value::from)
            .boxed(),
        _ => noise(),
    };
    prop_oneof![4 => shaped, 1 => noise()].boxed()
}

fn build(n: &Node) -> Erased {
    match n {
        Node::Str {
            min,
            max,
            prefix,
            suffix,
            email,
        } => {
            let mut s = vld::string();
            if *email {
                s = s.email();
            }
            if let Some(m) = min {
                s = s.min(*m);
            }
            if let Some(m) = max {
                s = s.max(*m);
            }
            if let Some(p) = prefix {
                s = s.starts_with(*p);
            }
            if let Some(p) = suffix {
                s = s.ends_with(*p);
            }
            Erased::new(s)
        }
        Node::Int { min, max, gt } => {
            let mut s = vld::number().int();
            if let Some(m) = min {
                s = s.min(*m);
            }
            if let Some(m) = max {
                s = s.max(*m);
            }
            if let Some(m) = gt {
                s = s.gt(*m);
            }
            Erased::new(s)
        }
        Node::Float { min, lt } => {
            let mut s = vld::number();
            if let Some(m) = min {
                s = s.min(*m);
            }
            if let Some(m) = lt {
                s = s.lt(*m);
            }
            Erased::new(s)
        }
        Node::Bool => Erased::new(vld::boolean()),
        Node::Lit(w) => Erased::new(vld::literal(*w)),
        Node::Enum(ws) => Erased::new(vld::enumeration(ws)),
        Node::Array {
            item,
            min,
            max,
            unique,
        } => {
            let mut s = vld::array(build(item));
            if let Some(m) = min {
                s = s.min_len(*m);
            }
            if let Some(m) = max {
                s = s.max_len(*m);
            }
            if *unique {
                s = s.unique();
            }
            Erased::new(s)
        }
        Node::Object { fields, strict } => {
            let mut o = vld::object();
            for (k, f) in fields {
                o = o.field(*k, build(f));
            }
            Erased::new(if *strict { o.strict() } else { o.strip() })
        }
        Node::Optional(i) => Erased::new(build(i).optional()),
        Node::Nullable(i) => Erased::new(build(i).nullable()),
        Node::Nullish(i) => Erased::new(build(i).nullish()),
        Node::Union(a, b) => Erased::new(vld::union(build(a), build(b))),
    }
}

/// Every `(key, value)` entry of every object anywhere in `v`.
fn entries(v: &Value, out: &mut Vec<(String, Value)>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                out.push((k.clone(), x.clone()));
                entries(x, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| entries(x, out)),
        _ => {}
    }
}

/// Where vld and its own JSON Schema knowingly differ (vld core, not this
/// crate): `optional()` also takes `null` while its JSON Schema is the inner
/// schema (optional = the key may be missing), and `number().int()` stops at
/// the `i64` range while its schema has no bound; a union with a nullable
/// branch (and no optional one) may be missing in vld but is listed in
/// `required`, like a `nullable()` field.
fn known_divergence(n: &Node, v: &Value) -> bool {
    match (n, v) {
        (Node::Optional(_), Value::Null) => true,
        (Node::Int { .. }, Value::Number(x)) => x.as_i64().is_none() && x.as_u64().is_some(),
        (Node::Nullable(_) | Node::Nullish(_), Value::Null) => false,
        (Node::Optional(i) | Node::Nullable(i) | Node::Nullish(i), v) => known_divergence(i, v),
        (Node::Union(a, b), v) => known_divergence(a, v) || known_divergence(b, v),
        (Node::Array { item, .. }, Value::Array(xs)) => {
            xs.iter().any(|x| known_divergence(item, x))
        }
        (Node::Object { fields, .. }, Value::Object(m)) => {
            fields.iter().any(|(k, f)| match m.get(*k) {
                Some(x) => known_divergence(f, x),
                None => optional_branch(f),
            })
        }
        _ => false,
    }
}

/// A union still listed in `required` (no branch is optional / nullish)
/// whose `nullable` branch takes the `null` a missing key parses as.
fn optional_branch(n: &Node) -> bool {
    fn skippable(n: &Node) -> bool {
        match n {
            Node::Optional(_) | Node::Nullish(_) => true,
            Node::Union(a, b) => skippable(a) || skippable(b),
            _ => false,
        }
    }
    fn takes_null(n: &Node) -> bool {
        match n {
            Node::Optional(_) | Node::Nullable(_) | Node::Nullish(_) => true,
            Node::Union(a, b) => takes_null(a) || takes_null(b),
            _ => false,
        }
    }
    matches!(n, Node::Union(..)) && !skippable(n) && takes_null(n)
}

fn check(n: &Node, v: &Value) -> Result<(), TestCaseError> {
    let schema = build(n);
    let js = schema.json_schema();
    let vld = schema.parse_value(v).map(drop).map_err(|e| pairs(&e));
    let ours = vld_schemars::validate_with_schema(&js, v).map_err(|e| pairs(&e));
    if !known_divergence(n, v) {
        prop_assert_eq!(
            vld.is_ok(),
            ours.is_ok(),
            "value {} schema {}: vld {:?}, validator {:?}",
            v,
            js,
            vld,
            ours
        );
    }

    // The schemars round trip keeps the document and the verdict.
    let s = vld_schemars::vld_to_schemars(&js);
    prop_assert_eq!(&vld_schemars::schemars_to_json(&s), &js);
    prop_assert_eq!(
        vld_schemars::validate_with_schemars(&s, v).is_ok(),
        ours.is_ok()
    );

    // OpenAPI 3.0: no 2020-12 keyword left, and a second pass changes nothing.
    let mut oas = js.clone();
    vld_schemars::to_openapi3(&mut oas);
    let mut again = oas.clone();
    vld_schemars::to_openapi3(&mut again);
    prop_assert_eq!(&again, &oas);
    let mut all = Vec::new();
    entries(&oas, &mut all);
    for (k, x) in &all {
        let left = match k.as_str() {
            "prefixItems" => true,
            "type" => x == "null" || x.as_array().is_some_and(|t| t.contains(&json!("null"))),
            "exclusiveMinimum" | "exclusiveMaximum" => !x.is_boolean(),
            _ => false,
        };
        prop_assert!(!left, "{} = {} left in {}", k, x, oas);
    }
    Ok(())
}

/// Given a random vld schema tree and an input, when the emitted JSON Schema
/// validates the input, then it agrees with vld (and through schemars too).
#[test]
fn validator_agrees_with_vld() {
    let cases = std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|c| c.parse().ok())
        .unwrap_or(1500);
    let mut runner = TestRunner::new(Config {
        cases,
        failure_persistence: None,
        ..Config::default()
    });
    let strategy = node().prop_flat_map(|n| {
        let i = input(&n);
        (Just(n), proptest::collection::vec(i, 1..6))
    });
    runner
        .run(&strategy, |(n, inputs)| {
            for v in &inputs {
                check(&n, v)?;
            }
            Ok(())
        })
        .unwrap();
}
