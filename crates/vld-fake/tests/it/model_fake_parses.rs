//! Model test: every fake value parses with the schema it was generated from,
//! and a seed always gives the same value.
//!
//! proptest builds a random, satisfiable vld schema tree (strings with a
//! format, or with lengths, prefixes and suffixes; integers with ranges and
//! `multipleOf`; floats with inclusive / exclusive bounds; booleans,
//! literals, enums; arrays with lengths and `unique`; tuples; records;
//! objects, strict or stripping; `optional` / `nullable` / `nullish`;
//! unions; `describe` / `with_default`) and a seed. vld itself is the oracle:
//! `fake_value_seeded(json_schema, seed)` must pass `parse_value`, twice the
//! same seed must give the same value, and a `FakeGen` over the same seeded
//! RNG must agree with `fake_value_seeded`.

use crate::common::Erased;

use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use rand::SeedableRng;
use vld::openapi::JsonSchema;
use vld::prelude::*;
use vld_fake::{FakeGen, fake_value_seeded};

/// String formats with their own generator.
const FORMATS: [&str; 33] = [
    "email",
    "url",
    "uuid",
    "uuid_v4",
    "ipv4",
    "ipv6",
    "hostname",
    "iso_date",
    "iso_datetime",
    "iso_time",
    "slug",
    "hex",
    "ulid",
    "nanoid",
    "cuid2",
    "semver",
    "mac",
    "jwt",
    "base64",
    "credit_card",
    "phone",
    "color",
    "alpha",
    "alphanumeric",
    "lowercase",
    "uppercase",
    "ascii",
    "emoji",
    "json",
    "cidr",
    "cron",
    "locale",
    "currency_code",
];

#[derive(Debug, Clone)]
enum Node {
    Format(&'static str),
    Str {
        min: usize,
        span: Option<usize>,
        prefix: Option<&'static str>,
        suffix: Option<&'static str>,
    },
    Int {
        min: i64,
        span: i64,
        multiple: Option<i64>,
    },
    Float {
        min: Option<f64>,
        exclusive: bool,
        width: f64,
    },
    Bool,
    Lit(&'static str),
    Enum(Vec<&'static str>),
    Array {
        item: Box<Node>,
        min: usize,
        span: usize,
        unique: bool,
    },
    Tuple(Box<Node>, Box<Node>),
    Record(Box<Node>),
    Object {
        fields: Vec<(&'static str, Node)>,
        strict: bool,
    },
    Optional(Box<Node>),
    Nullable(Box<Node>),
    Nullish(Box<Node>),
    Describe(Box<Node>),
    Union(Box<Node>, Box<Node>),
}

const KEYS: [&str; 4] = ["name", "email", "city", "price"];

fn leaf() -> impl Strategy<Value = Node> {
    let lit = || proptest::option::of(proptest::sample::select(vec!["ab", "z_", "pre"]));
    prop_oneof![
        proptest::sample::select(FORMATS.to_vec()).prop_map(Node::Format),
        (0usize..6, proptest::option::of(0usize..8), lit(), lit()).prop_map(
            |(min, span, prefix, suffix)| Node::Str {
                min,
                span,
                prefix,
                suffix
            }
        ),
        (-60i64..60, 0i64..40, proptest::option::of(1i64..6)).prop_map(|(min, span, multiple)| {
            Node::Int {
                min,
                span: span.max(multiple.unwrap_or(1)),
                multiple,
            }
        }),
        (
            proptest::option::of(-100.0f64..100.0),
            any::<bool>(),
            proptest::sample::select(vec![0.001, 0.5, 10.0, 1e6])
        )
            .prop_map(|(min, exclusive, width)| Node::Float {
                min,
                exclusive,
                width
            }),
        Just(Node::Bool),
        proptest::sample::select(vec!["a", "x y"]).prop_map(Node::Lit),
        proptest::sample::subsequence(vec!["red", "green", "blue"], 1..4).prop_map(Node::Enum),
    ]
}

/// `unique` needs an item domain larger than the array.
fn wide(n: &Node) -> bool {
    match n {
        Node::Format(_) => true,
        Node::Str { span, .. } => span.is_none_or(|s| s >= 3),
        Node::Int { span, multiple, .. } => span / multiple.unwrap_or(1) >= 8,
        Node::Float { width, .. } => *width >= 10.0,
        _ => false,
    }
}

fn node() -> impl Strategy<Value = Node> {
    leaf().prop_recursive(3, 24, 3, |inner| {
        prop_oneof![
            (inner.clone(), 0usize..3, 0usize..4, any::<bool>()).prop_map(
                |(i, min, span, unique)| {
                    let unique = unique && wide(&i);
                    Node::Array {
                        item: Box::new(i),
                        min,
                        span,
                        unique,
                    }
                }
            ),
            (inner.clone(), inner.clone()).prop_map(|(a, b)| Node::Tuple(Box::new(a), Box::new(b))),
            inner.clone().prop_map(|i| Node::Record(Box::new(i))),
            (
                proptest::collection::vec(inner.clone(), 0..=4),
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
        ]
    })
}

fn format(f: &str) -> vld::primitives::ZString {
    let s = vld::string();
    match f {
        "email" => s.email(),
        "url" => s.url(),
        "uuid" => s.uuid(),
        "uuid_v4" => s.uuid_v4(),
        "ipv4" => s.ipv4(),
        "ipv6" => s.ipv6(),
        "hostname" => s.hostname(),
        "iso_date" => s.iso_date(),
        "iso_datetime" => s.iso_datetime(),
        "iso_time" => s.iso_time(),
        "slug" => s.slug(),
        "hex" => s.hex(),
        "ulid" => s.ulid(),
        "nanoid" => s.nanoid(),
        "cuid2" => s.cuid2(),
        "semver" => s.semver(),
        "mac" => s.mac(),
        "jwt" => s.jwt(),
        "base64" => s.base64(),
        "credit_card" => s.credit_card(),
        "phone" => s.phone(),
        "color" => s.color(),
        "alpha" => s.alpha(),
        "alphanumeric" => s.alphanumeric(),
        "lowercase" => s.lowercase(),
        "uppercase" => s.uppercase(),
        "ascii" => s.ascii(),
        "emoji" => s.emoji(),
        "json" => s.json(),
        "cidr" => s.cidr(),
        "cron" => s.cron(),
        "locale" => s.locale(),
        "currency_code" => s.currency_code(),
        other => unreachable!("{other}"),
    }
}

fn build(n: &Node) -> Erased {
    match n {
        Node::Format(f) => Erased::new(format(f)),
        Node::Str {
            min,
            span,
            prefix,
            suffix,
        } => {
            let literals = prefix.map_or(0, str::len) + suffix.map_or(0, str::len);
            let mut s = vld::string().min(*min);
            if let Some(span) = span {
                s = s.max((*min).max(literals) + span);
            }
            if let Some(p) = prefix {
                s = s.starts_with(*p);
            }
            if let Some(p) = suffix {
                s = s.ends_with(*p);
            }
            Erased::new(s)
        }
        Node::Int {
            min,
            span,
            multiple,
        } => {
            let mut s = vld::number().int().min(*min).max(min + span);
            if let Some(m) = multiple {
                s = s.multiple_of(*m);
            }
            Erased::new(s)
        }
        Node::Float {
            min,
            exclusive,
            width,
        } => {
            let mut s = vld::number();
            if let Some(m) = min {
                s = if *exclusive {
                    s.gt(*m).lt(m + width)
                } else {
                    s.min(*m).max(m + width)
                };
            }
            Erased::new(s)
        }
        Node::Bool => Erased::new(vld::boolean()),
        Node::Lit(w) => Erased::new(vld::literal(*w)),
        Node::Enum(ws) => Erased::new(vld::enumeration(ws)),
        Node::Array {
            item,
            min,
            span,
            unique,
        } => {
            let mut s = vld::array(build(item)).min_len(*min).max_len(min + span);
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
        Node::Optional(i) => Erased::new(build(i).optional()),
        Node::Nullable(i) => Erased::new(build(i).nullable()),
        Node::Nullish(i) => Erased::new(build(i).nullish()),
        Node::Describe(i) => Erased::new(build(i).describe("described")),
        Node::Union(a, b) => Erased::new(vld::union(build(a), build(b))),
    }
}

fn check(n: &Node, seed: u64) -> Result<(), TestCaseError> {
    let schema = build(n);
    let js = schema.json_schema();
    let v = fake_value_seeded(&js, seed);
    if let Err(e) = schema.parse_value(&v) {
        return Err(TestCaseError::fail(format!(
            "fake {v} fails its schema {js}: {e}"
        )));
    }
    prop_assert_eq!(
        &fake_value_seeded(&js, seed),
        &v,
        "seed {} not reproducible",
        seed
    );
    let mut generator = FakeGen::with_rng(rand::rngs::StdRng::seed_from_u64(seed));
    prop_assert_eq!(&generator.value(&js), &v);
    Ok(())
}

/// Given a random satisfiable vld schema and a seed, when a fake value is
/// generated from its JSON Schema, then vld accepts it and the seed
/// reproduces it.
#[test]
fn fake_values_parse_and_are_reproducible() {
    let cases = std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|c| c.parse().ok())
        .unwrap_or(1500);
    let mut runner = TestRunner::new(Config {
        cases,
        failure_persistence: None,
        ..Config::default()
    });
    let strategy = (node(), proptest::collection::vec(any::<u64>(), 1..6));
    runner
        .run(&strategy, |(n, seeds)| {
            for seed in seeds {
                check(&n, seed)?;
            }
            Ok(())
        })
        .unwrap();
}
