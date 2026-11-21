//! Model test: the utoipa schema of any vld schema documents the same
//! constraints.
//!
//! proptest builds a random vld schema tree (strings with lengths, formats,
//! prefixes and suffixes; integers and floats with inclusive / exclusive
//! bounds and `multipleOf`; booleans, literals, enums; arrays with lengths and
//! `unique`; tuples; records; objects, strict or stripping, with descriptions
//! and defaults; `optional` / `nullable` / `nullish`; unions). A small model
//! of the documented mapping predicts the serialized utoipa schema from vld's
//! JSON Schema (`const` → one-value `enum`, whole floats as integers; every
//! other keyword vld emits is kept), and for objects the `IntoParams`
//! parameters (one per property, required iff listed in `required`, the
//! non-null branch of a nullable union as the schema).

use crate::common::Erased;

use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use serde_json::{Map, Value, json};
use vld::openapi::JsonSchema;
use vld::prelude::*;
use vld_utoipa::utoipa::openapi::Required;
use vld_utoipa::utoipa::openapi::path::ParameterIn;
use vld_utoipa::{json_schema_to_params, json_schema_to_schema};

#[derive(Debug, Clone)]
enum Node {
    Str {
        min: Option<usize>,
        max: Option<usize>,
        format: Option<&'static str>,
        prefix: Option<&'static str>,
        suffix: Option<&'static str>,
    },
    Int {
        min: Option<i64>,
        gt: Option<i64>,
        multiple: Option<i64>,
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
    Tuple(Box<Node>, Box<Node>),
    Record(Box<Node>),
    Object {
        fields: Vec<(&'static str, Node)>,
        strict: bool,
    },
    Describe(Box<Node>),
    Optional(Box<Node>),
    Nullable(Box<Node>),
    Nullish(Box<Node>),
    Union(Box<Node>, Box<Node>),
}

const KEYS: [&str; 3] = ["a", "b_c", "d"];

fn leaf() -> impl Strategy<Value = Node> {
    let len = || proptest::option::of(0usize..5);
    let lit = || proptest::option::of(proptest::sample::select(vec!["a", "zz"]));
    let int = || proptest::option::of(-3i64..8);
    prop_oneof![
        (
            len(),
            len(),
            proptest::option::of(proptest::sample::select(vec!["email", "uuid", "url"])),
            lit(),
            lit()
        )
            .prop_map(|(min, max, format, prefix, suffix)| Node::Str {
                min,
                max,
                format,
                prefix,
                suffix
            }),
        (int(), int(), proptest::option::of(2i64..5)).prop_map(|(min, gt, multiple)| Node::Int {
            min,
            gt,
            multiple
        }),
        (
            proptest::option::of(proptest::sample::select(vec![-1.5, 0.0, 2.0])),
            proptest::option::of(proptest::sample::select(vec![0.5, 3.0, 1e18])),
        )
            .prop_map(|(min, lt)| Node::Float { min, lt }),
        Just(Node::Bool),
        proptest::sample::select(vec!["a", "b\"c"]).prop_map(Node::Lit),
        proptest::sample::subsequence(vec!["a", "ab", "abc"], 1..4).prop_map(Node::Enum),
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
            (inner.clone(), inner.clone()).prop_map(|(a, b)| Node::Tuple(Box::new(a), Box::new(b))),
            inner.clone().prop_map(|i| Node::Record(Box::new(i))),
            (
                proptest::collection::vec(inner.clone(), 0..=3),
                any::<bool>()
            )
                .prop_map(|(fs, strict)| Node::Object {
                    fields: KEYS.iter().copied().zip(fs).collect(),
                    strict,
                }),
            inner.clone().prop_map(|i| Node::Describe(Box::new(i))),
            inner.clone().prop_map(|i| Node::Optional(Box::new(i))),
            inner.clone().prop_map(|i| Node::Nullable(Box::new(i))),
            inner.clone().prop_map(|i| Node::Nullish(Box::new(i))),
            (inner.clone(), inner).prop_map(|(a, b)| Node::Union(Box::new(a), Box::new(b))),
        ]
    })
}

fn build(n: &Node) -> Erased {
    match n {
        Node::Str {
            min,
            max,
            format,
            prefix,
            suffix,
        } => {
            let mut s = vld::string();
            s = match *format {
                Some("email") => s.email(),
                Some("uuid") => s.uuid(),
                Some(_) => s.url(),
                None => s,
            };
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
        Node::Int { min, gt, multiple } => {
            let mut s = vld::number().int();
            if let Some(m) = min {
                s = s.min(*m);
            }
            if let Some(m) = gt {
                s = s.gt(*m);
            }
            if let Some(m) = multiple {
                s = s.multiple_of(*m);
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
        Node::Tuple(a, b) => Erased::new((build(a), build(b))),
        Node::Record(v) => Erased::new(vld::record(build(v))),
        Node::Object { fields, strict } => {
            let mut o = vld::object();
            for (k, f) in fields {
                o = o.field(*k, build(f));
            }
            Erased::new(if *strict { o.strict() } else { o.strip() })
        }
        Node::Describe(i) => Erased::new(build(i).describe("about \"it\"")),
        Node::Optional(i) => Erased::new(build(i).optional()),
        Node::Nullable(i) => Erased::new(build(i).nullable()),
        Node::Nullish(i) => Erased::new(build(i).nullish()),
        Node::Union(a, b) => Erased::new(vld::union(build(a), build(b))),
    }
}

/// Whole floats as integers (utoipa's `Number` has no `2.0`).
fn numbers(v: &Value) -> Value {
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(f) if n.is_f64() && f.fract() == 0.0 && f.abs() < isize::MAX as f64 => {
                json!(f as i64)
            }
            _ => v.clone(),
        },
        Value::Array(a) => Value::Array(a.iter().map(numbers).collect()),
        Value::Object(m) => Value::Object(m.iter().map(|(k, x)| (k.clone(), numbers(x))).collect()),
        _ => v.clone(),
    }
}

/// Annotations and extensions stay on a composite; other keywords next to
/// `oneOf` / `anyOf` / `allOf` move into it.
fn stays_outside(k: &str) -> bool {
    ["title", "description", "default", "example"].contains(&k) || k.starts_with("x-")
}

/// The model: vld's JSON Schema as utoipa serializes it.
fn predict(js: &Value) -> Value {
    let Some(m) = js.as_object() else {
        return js.clone();
    };
    let (outside, siblings): (Map<String, Value>, Map<String, Value>) = m
        .iter()
        .filter(|(k, _)| !["oneOf", "anyOf", "allOf"].contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .partition(|(k, _)| stays_outside(k));
    if !siblings.is_empty() {
        // Siblings of a union apply to every non-null branch.
        for key in ["oneOf", "anyOf"] {
            if let Some(Value::Array(branches)) = m.get(key) {
                let mut out = outside.clone();
                let branches = branches
                    .iter()
                    .map(|b| match b.as_object() {
                        Some(o) if b["type"] != "null" => {
                            let mut merged = siblings.clone();
                            merged.extend(o.clone());
                            Value::Object(merged)
                        }
                        _ => b.clone(),
                    })
                    .collect();
                out.insert(key.into(), Value::Array(branches));
                return predict(&Value::Object(out));
            }
        }
        // A typed base next to `allOf` becomes its first member.
        if let Some(Value::Array(parts)) = m.get("allOf") {
            let mut out = outside;
            let mut all = vec![Value::Object(siblings)];
            all.extend(parts.iter().cloned());
            out.insert("allOf".into(), Value::Array(all));
            return predict(&Value::Object(out));
        }
    }
    let mut out = Map::new();
    for (k, v) in m {
        let v = match k.as_str() {
            "properties" => Value::Object(
                v.as_object()
                    .unwrap()
                    .iter()
                    .map(|(k, x)| (k.clone(), predict(x)))
                    .collect(),
            ),
            "items" | "additionalProperties" | "propertyNames" => predict(v),
            "anyOf" | "oneOf" | "allOf" | "prefixItems" => {
                Value::Array(v.as_array().unwrap().iter().map(predict).collect())
            }
            _ => numbers(v),
        };
        match k.as_str() {
            // utoipa omits an empty `properties` (same meaning).
            "properties" if v.as_object().is_some_and(Map::is_empty) => {}
            "const" if !m.contains_key("enum") => {
                out.insert("enum".into(), json!([v]));
            }
            _ => {
                out.insert(k.clone(), v);
            }
        }
    }
    Value::Object(out)
}

/// utoipa may add keys with their default value; the model only sees what vld
/// emits.
fn without_defaults(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .filter(|(k, x)| !(k.as_str() == "uniqueItems" && **x == json!(false)))
                .map(|(k, x)| (k.clone(), without_defaults(x)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(without_defaults).collect()),
        _ => v.clone(),
    }
}

/// The non-null branch of a nullable union, with the union's own keys.
fn unwrap_nullable(v: &Value) -> Value {
    for key in ["anyOf", "oneOf"] {
        if let Some(branches) = v.get(key).and_then(Value::as_array) {
            let rest: Vec<&Value> = branches.iter().filter(|b| b["type"] != "null").collect();
            if let ([one], true) = (rest.as_slice(), rest.len() < branches.len()) {
                let mut one = (*one).clone();
                if let Some(o) = one.as_object_mut() {
                    for (k, x) in v.as_object().unwrap() {
                        if k != key {
                            o.entry(k.clone()).or_insert_with(|| x.clone());
                        }
                    }
                }
                return one;
            }
        }
    }
    v.clone()
}

fn check(n: &Node) -> Result<(), TestCaseError> {
    let js = build(n).json_schema();
    let got = without_defaults(&serde_json::to_value(json_schema_to_schema(&js)).unwrap());
    prop_assert_eq!(&got, &predict(&js), "vld {}", js);

    if js.get("properties").is_some() {
        let required: Vec<&str> = js["required"]
            .as_array()
            .map(|r| r.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let params = json_schema_to_params(&js, ParameterIn::Header);
        let props = js["properties"].as_object().unwrap();
        prop_assert_eq!(params.len(), props.len());
        for (p, (name, schema)) in params.iter().zip(props) {
            prop_assert_eq!(&p.name, name);
            prop_assert!(matches!(p.parameter_in, ParameterIn::Header));
            let want = if required.contains(&name.as_str()) {
                Required::True
            } else {
                Required::False
            };
            prop_assert!(p.required == want, "{} in {}", name, js);
            let inner = unwrap_nullable(schema);
            prop_assert_eq!(
                p.description.as_deref(),
                inner.get("description").and_then(Value::as_str)
            );
            let schema = serde_json::to_value(p.schema.as_ref().unwrap()).unwrap();
            prop_assert_eq!(without_defaults(&schema), predict(&inner), "{}", name);
        }
    } else {
        prop_assert!(json_schema_to_params(&js, ParameterIn::Query).is_empty());
    }
    Ok(())
}

/// Given a random vld schema tree, when it is converted to utoipa (schema and
/// parameters), then the output matches the model of the mapping.
#[test]
fn utoipa_schemas_document_the_vld_rules() {
    let cases = std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|c| c.parse().ok())
        .unwrap_or(1500);
    let mut runner = TestRunner::new(Config {
        cases,
        failure_persistence: None,
        ..Config::default()
    });
    runner.run(&node(), |n| check(&n)).unwrap();
}
