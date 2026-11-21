//! Model test: the generated Zod 3, Zod 4, Valibot and TypeScript code of a
//! random vld schema, predicted exactly.
//!
//! proptest builds a random vld schema tree (strings with lengths and
//! `email`, integers and floats with bounds, booleans, literals, enums,
//! arrays with lengths, objects — strict or stripping, with quoted and bare
//! keys — `optional` / `nullable` / `nullish`, `describe`, `with_default`,
//! unions, which become discriminated unions when every option is a plain
//! object with a distinct literal tag). A small model of each generator's
//! documented mapping predicts the code for all four targets from the tree;
//! the generated code must be exactly that. So every object field appears,
//! optional fields are `.optional()` / `v.optional(…)` / `key?:`, nullable ones
//! `.nullable()` / `v.nullable(…)` / `| null`, and so on.

use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use vld::openapi::JsonSchema;
use vld::prelude::*;

use crate::common::Erased;

#[derive(Debug, Clone)]
enum Node {
    Str {
        min: Option<usize>,
        max: Option<usize>,
        email: bool,
    },
    Int {
        min: Option<i64>,
        max: Option<i64>,
    },
    Float {
        min: Option<f64>,
    },
    Bool,
    Lit(&'static str),
    Enum(Vec<&'static str>),
    Array {
        item: Box<Node>,
        min: Option<usize>,
        max: Option<usize>,
    },
    Object {
        fields: Vec<(&'static str, Node)>,
        strict: bool,
    },
    Optional(Box<Node>),
    Nullable(Box<Node>),
    Nullish(Box<Node>),
    Describe(Box<Node>),
    DefaultStr,
    Union(Box<Node>, Box<Node>),
}

/// Sorted, so declaration order is also the map's key order.
const KEYS: [&str; 3] = ["a", "b_c", "d-e"];

fn leaf() -> impl Strategy<Value = Node> {
    let len = || proptest::option::of(0usize..4);
    let int = || proptest::option::of(-3i64..8);
    prop_oneof![
        (len(), len(), any::<bool>()).prop_map(|(min, max, email)| Node::Str { min, max, email }),
        (int(), int()).prop_map(|(min, max)| Node::Int { min, max }),
        proptest::option::of(proptest::sample::select(vec![-1.5, 0.25, 2.0]))
            .prop_map(|min| Node::Float { min }),
        Just(Node::Bool),
        proptest::sample::select(vec!["a", "q\"t"]).prop_map(Node::Lit),
        proptest::sample::subsequence(vec!["a", "b", "c"], 1..4).prop_map(Node::Enum),
        Just(Node::DefaultStr),
    ]
}

fn node() -> impl Strategy<Value = Node> {
    leaf().prop_recursive(3, 24, 3, |inner| {
        let len = || proptest::option::of(0usize..4);
        // A tagged object: the first field is a literal (a discriminator).
        let tagged = (
            proptest::sample::select(vec!["a", "b"]),
            proptest::collection::vec(inner.clone(), 0..=2),
        )
            .prop_map(|(tag, rest)| {
                let mut fields = vec![("a", Node::Lit(tag))];
                fields.extend(KEYS[1..].iter().copied().zip(rest));
                Node::Object {
                    fields,
                    strict: false,
                }
            });
        prop_oneof![
            (inner.clone(), len(), len()).prop_map(|(i, min, max)| Node::Array {
                item: Box::new(i),
                min,
                max
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
            inner.clone().prop_map(|i| Node::Describe(Box::new(i))),
            (inner.clone(), inner).prop_map(|(a, b)| Node::Union(Box::new(a), Box::new(b))),
            (tagged.clone(), tagged).prop_map(|(a, b)| Node::Union(Box::new(a), Box::new(b))),
        ]
    })
}

fn build(n: &Node) -> Erased {
    match n {
        Node::Str { min, max, email } => {
            let mut s = vld::string();
            if let Some(m) = min {
                s = s.min(*m);
            }
            if let Some(m) = max {
                s = s.max(*m);
            }
            if *email {
                s = s.email();
            }
            Erased::new(s)
        }
        Node::Int { min, max } => {
            let mut s = vld::number().int();
            if let Some(m) = min {
                s = s.min(*m);
            }
            if let Some(m) = max {
                s = s.max(*m);
            }
            Erased::new(s)
        }
        Node::Float { min } => Erased::new(match min {
            Some(m) => vld::number().min(*m),
            None => vld::number(),
        }),
        Node::Bool => Erased::new(vld::boolean()),
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
        Node::Describe(i) => Erased::new(build(i).describe("d \"q\"")),
        Node::DefaultStr => Erased::new(vld::string().with_default("dflt".to_string())),
        Node::Union(a, b) => Erased::new(vld::union(build(a), build(b))),
    }
}

// ---------------------------------------------------------------------------
// The model
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum T {
    Zod,
    Zod4,
    Valibot,
}

const DESC: &str = r#""d \"q\"""#;

fn q(s: &str) -> String {
    serde_json::Value::from(s).to_string()
}

fn key(k: &str) -> String {
    if k.contains('-') { q(k) } else { k.to_string() }
}

fn num(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// `base` with method-chain (Zod) / `v.pipe` (Valibot) actions.
fn pipe(t: T, base: String, acts: &[String]) -> String {
    if acts.is_empty() {
        return base;
    }
    match t {
        T::Valibot => format!(
            "v.pipe({base}, {})",
            acts.iter()
                .map(|a| format!("v.{a}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => acts.iter().fold(base, |e, a| format!("{e}.{a}")),
    }
}

fn act(t: T, zod: &str, valibot: &str, arg: impl std::fmt::Display) -> String {
    match t {
        T::Valibot => format!("{valibot}({arg})"),
        _ => format!("{zod}({arg})"),
    }
}

fn ns(t: T) -> &'static str {
    if t == T::Valibot { "v" } else { "z" }
}

fn nullable(t: T, e: String) -> String {
    match t {
        T::Valibot => format!("v.nullable({e})"),
        _ => format!("{e}.nullable()"),
    }
}

/// The schema emits a `default` (the field is then optional on input).
fn has_default(n: &Node) -> bool {
    match n {
        Node::DefaultStr => true,
        Node::Describe(i) | Node::Optional(i) => has_default(i),
        _ => false,
    }
}

/// vld's object `required`: not optional, nullish or defaulted; a union
/// only when every branch is.
fn required(n: &Node) -> bool {
    match n {
        Node::Optional(_) | Node::Nullish(_) | Node::DefaultStr => false,
        Node::Describe(i) => required(i),
        Node::Union(a, b) => required(a) && required(b),
        _ => true,
    }
}

/// The JSON Schema is `anyOf: [inner, null]`.
fn null_union(n: &Node) -> Option<&Node> {
    match n {
        Node::Nullable(i) | Node::Nullish(i) => Some(i),
        Node::Optional(i) | Node::Describe(i) => null_union(i),
        _ => None,
    }
}

/// A union option as a plain object (no description for Valibot, whose
/// `v.variant` takes no `v.pipe`): its fields.
fn plain_object(t: T, n: &Node) -> Option<&[(&'static str, Node)]> {
    match n {
        Node::Object { fields, .. } => Some(fields),
        Node::Optional(i) => plain_object(t, i),
        Node::Describe(i) if t != T::Valibot => plain_object(t, i),
        _ => None,
    }
}

/// The string literal a required field always holds.
fn literal(n: &Node) -> Option<&'static str> {
    match n {
        Node::Lit(v) => Some(v),
        Node::Enum(ws) if ws.len() == 1 => Some(ws[0]),
        Node::Describe(i) => literal(i),
        _ => None,
    }
}

/// The discriminator of a union: the first required key of the first option
/// that is a distinct string literal in every option (all plain objects).
fn discriminator(t: T, opts: &[&Node]) -> Option<&'static str> {
    let objects: Vec<_> = opts
        .iter()
        .map(|o| plain_object(t, o))
        .collect::<Option<_>>()?;
    let tag_in = |fields: &[(&'static str, Node)], k: &str| {
        fields
            .iter()
            .find(|(f, _)| *f == k)
            .filter(|(_, n)| required(n))
            .and_then(|(_, n)| literal(n))
    };
    objects[0]
        .iter()
        .filter(|(_, n)| required(n))
        .map(|(k, _)| *k)
        .find(|k| {
            let mut seen = Vec::new();
            objects.iter().all(|fields| match tag_in(fields, k) {
                Some(v) if !seen.contains(&v) => {
                    seen.push(v);
                    true
                }
                _ => false,
            })
        })
}

/// Base expression and its actions (method chain / `v.pipe` items), as the
/// generator builds them before a default wraps the whole.
fn core(t: T, n: &Node) -> (String, Vec<String>) {
    let ns = ns(t);
    match n {
        Node::Str { min, max, email } => {
            let mut acts = Vec::new();
            let fmt4 = t == T::Zod4 && *email;
            if let Some(m) = min {
                acts.push(act(t, "min", "minLength", m));
            }
            if let Some(m) = max {
                acts.push(act(t, "max", "maxLength", m));
            }
            if *email && !fmt4 {
                acts.push("email()".to_string());
            }
            let base = if fmt4 {
                "z.email()".to_string()
            } else {
                format!("{ns}.string()")
            };
            (base, acts)
        }
        Node::Int { min, max } => {
            let mut acts = Vec::new();
            if t != T::Zod4 {
                acts.push(act(t, "int", "integer", ""));
            }
            if let Some(m) = min {
                acts.push(act(t, "min", "minValue", m));
            }
            if let Some(m) = max {
                acts.push(act(t, "max", "maxValue", m));
            }
            let base = if t == T::Zod4 {
                "z.int()".to_string()
            } else {
                format!("{ns}.number()")
            };
            (base, acts)
        }
        Node::Float { min } => (
            format!("{ns}.number()"),
            min.iter()
                .map(|m| act(t, "min", "minValue", num(*m)))
                .collect(),
        ),
        Node::Bool => (format!("{ns}.boolean()"), vec![]),
        Node::Lit(w) => (format!("{ns}.literal({})", q(w)), vec![]),
        Node::Enum(ws) if ws.len() == 1 => (format!("{ns}.literal({})", q(ws[0])), vec![]),
        Node::Enum(ws) => (
            format!(
                "{ns}.union([{}])",
                ws.iter()
                    .map(|w| format!("{ns}.literal({})", q(w)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            vec![],
        ),
        Node::Array { item, min, max } => {
            let acts = match (min, max) {
                (Some(a), Some(b)) if a == b => vec![act(t, "length", "length", a)],
                _ => min
                    .iter()
                    .map(|m| act(t, "min", "minLength", m))
                    .chain(max.iter().map(|m| act(t, "max", "maxLength", m)))
                    .collect(),
            };
            (format!("{ns}.array({})", expect(t, item)), acts)
        }
        Node::Object { fields, strict } => {
            let entries: Vec<String> = fields
                .iter()
                .map(|(k, f)| {
                    let mut e = expect(t, f);
                    if !required(f) && !has_default(f) {
                        e = match t {
                            T::Valibot => format!("v.optional({e})"),
                            _ => format!("{e}.optional()"),
                        };
                    }
                    format!("  {}: {e}", key(k))
                })
                .collect();
            let body = if entries.is_empty() {
                "{}".to_string()
            } else {
                format!("{{\n{}\n}}", entries.join(",\n"))
            };
            let e = match (t, strict) {
                (T::Zod, true) => format!("z.object({body}).strict()"),
                (T::Zod4, true) => format!("z.strictObject({body})"),
                (T::Valibot, true) => format!("v.strictObject({body})"),
                (_, false) => format!("{ns}.object({body})"),
            };
            (e, vec![])
        }
        Node::Optional(i) => core(t, i),
        // Already `anyOf [.., null]`: vld does not wrap it again.
        Node::Nullable(i) | Node::Nullish(i) if null_union(i).is_some() => core(t, i),
        Node::Nullable(i) | Node::Nullish(i) => (nullable(t, expect(t, i)), vec![]),
        Node::Describe(i) => {
            let (b, mut acts) = core(t, undescribed(i));
            acts.push(act(t, "describe", "description", DESC));
            (b, acts)
        }
        Node::DefaultStr => (format!("{ns}.string()"), vec![]),
        Node::Union(a, b) => {
            let opts = [a.as_ref(), b.as_ref()];
            let parts: Vec<String> = opts.iter().map(|o| expect(t, o)).collect();
            let e = match (discriminator(t, &opts), t) {
                (Some(k), T::Valibot) => format!("v.variant({}, [{}])", q(k), parts.join(", ")),
                (Some(k), _) => format!("z.discriminatedUnion({}, [{}])", q(k), parts.join(", ")),
                (None, _) => format!("{ns}.union([{}])", parts.join(", ")),
            };
            (e, vec![])
        }
    }
}

/// The last `describe` wins; `optional` adds no keyword.
fn undescribed(n: &Node) -> &Node {
    match n {
        Node::Describe(i) | Node::Optional(i) => undescribed(i),
        _ => n,
    }
}

/// The generated expression: the piped core, wrapped by a default.
fn expect(t: T, n: &Node) -> String {
    let (base, acts) = core(t, n);
    let e = pipe(t, base, &acts);
    if !has_default(n) {
        return e;
    }
    match t {
        T::Valibot => format!("v.nullish({e}, \"dflt\")"),
        _ => format!("{e}.nullish().transform((v) => v ?? \"dflt\")"),
    }
}

/// TypeScript type and whether it is a bare union (needs parens in `T[]`).
fn ts(n: &Node) -> (String, bool) {
    match n {
        Node::Str { .. } | Node::DefaultStr => ("string".into(), false),
        Node::Int { .. } | Node::Float { .. } => ("number".into(), false),
        Node::Bool => ("boolean".into(), false),
        Node::Lit(w) => (q(w), false),
        Node::Enum(ws) => (
            ws.iter().map(|w| q(w)).collect::<Vec<_>>().join(" | "),
            ws.len() > 1,
        ),
        Node::Array { item, .. } => {
            let (e, union) = ts(item);
            (
                if union {
                    format!("({e})[]")
                } else {
                    format!("{e}[]")
                },
                false,
            )
        }
        Node::Object { fields, strict } => {
            if fields.is_empty() {
                let v = if *strict { "never" } else { "unknown" };
                return (format!("Record<string, {v}>"), false);
            }
            let lines: Vec<String> = fields
                .iter()
                .map(|(k, f)| {
                    let opt = if required(f) { "" } else { "?" };
                    format!("  {}{opt}: {};", key(k), ts(f).0.replace('\n', "\n  "))
                })
                .collect();
            (format!("{{\n{}\n}}", lines.join("\n")), false)
        }
        Node::Optional(i) | Node::Describe(i) => ts(i),
        Node::Nullable(i) | Node::Nullish(i) if null_union(i).is_some() => ts(i),
        Node::Nullable(i) | Node::Nullish(i) => union_of(vec![ts(i), ("null".into(), false)]),
        Node::Union(a, b) => union_of(vec![ts(a), ts(b)]),
    }
}

/// `A | B`, deduplicated by text.
fn union_of(parts: Vec<(String, bool)>) -> (String, bool) {
    let mut out: Vec<(String, bool)> = Vec::new();
    for p in parts {
        for piece in split_union(&p) {
            if !out.iter().any(|o| o.0 == piece.0) {
                out.push(piece);
            }
        }
    }
    if out.len() == 1 {
        return out.remove(0);
    }
    (
        out.into_iter().map(|p| p.0).collect::<Vec<_>>().join(" | "),
        true,
    )
}

/// A union's members are deduplicated against the outer union as whole
/// texts, not flattened: keep it as one piece.
fn split_union(p: &(String, bool)) -> Vec<(String, bool)> {
    vec![p.clone()]
}

fn check(n: &Node) -> Result<(), TestCaseError> {
    let schema = build(n);
    let js = schema.json_schema();
    for (t, got) in [
        (T::Zod, vld::ts::to_zod(&schema)),
        (T::Zod4, vld::ts::to_zod4(&schema)),
        (T::Valibot, vld::ts::to_valibot(&schema)),
    ] {
        prop_assert_eq!(&got, &expect(t, n), "{:?} for {}", t, js);
    }
    prop_assert_eq!(vld::ts::to_ts_type(&schema), ts(n).0, "ts for {}", js);
    Ok(())
}

/// Given a random vld schema tree, when code is generated for Zod 3, Zod 4,
/// Valibot and TypeScript, then each is exactly the model's prediction.
#[test]
fn generated_code_matches_the_model() {
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
