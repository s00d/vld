//! Model test: number schemas of every type with random checks and options.
//!
//! proptest picks a type (`number()`, `f32()`, `number().int()`, `i8()` …
//! `u64()`), a list of checks (`min` / `gte`, `max`, `gt`, `lt`, `positive`,
//! `negative`, `non_negative`, `non_positive`, `multiple_of`, `safe`), and the
//! options `clamp`, `coerce`, `type_error`, `int_error`, `with_messages`, then
//! an input (integers at and around every bound and type range, `2^53 ± 1`,
//! `u64::MAX`, fractions, huge floats, numeric / non-numeric strings,
//! booleans, `null`). The model predicts the output, or every issue code in
//! order, and the message of every issue a custom message applies to.

use crate::common::*;

use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use serde_json::{Value, json};
use vld::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Ty {
    F64,
    F32,
    /// `number().int()`: `i64` without a JSON Schema format.
    Int,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
}

impl Ty {
    fn range(self) -> Option<(i128, i128)> {
        Some(match self {
            Ty::F64 | Ty::F32 => return None,
            Ty::Int | Ty::I64 => (i64::MIN.into(), i64::MAX.into()),
            Ty::I8 => (i8::MIN.into(), i8::MAX.into()),
            Ty::I16 => (i16::MIN.into(), i16::MAX.into()),
            Ty::I32 => (i32::MIN.into(), i32::MAX.into()),
            Ty::U8 => (0, u8::MAX.into()),
            Ty::U16 => (0, u16::MAX.into()),
            Ty::U32 => (0, u32::MAX.into()),
            Ty::U64 => (0, u64::MAX.into()),
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum Check {
    Min(i64),
    /// `gte`: the alias of `min`.
    Gte(i64),
    Max(i64),
    Gt(i64),
    Lt(i64),
    Positive,
    Negative,
    NonNegative,
    NonPositive,
    MultipleOf(i64),
    Safe,
}

impl Check {
    fn key(self) -> &'static str {
        match self {
            Check::Min(_) | Check::Gte(_) | Check::Gt(_) => "too_small",
            Check::Max(_) | Check::Lt(_) => "too_big",
            Check::Positive => "not_positive",
            Check::Negative => "not_negative",
            Check::NonNegative => "not_non_negative",
            Check::NonPositive => "not_non_positive",
            Check::MultipleOf(_) => "not_multiple_of",
            Check::Safe => "not_safe",
        }
    }

    /// The issue code key of a failure.
    fn code(self) -> &'static str {
        match self {
            Check::Min(_) | Check::Gte(_) | Check::Gt(_) | Check::Positive | Check::NonNegative => {
                "too_small"
            }
            Check::Max(_) | Check::Lt(_) | Check::Negative | Check::NonPositive => "too_big",
            Check::MultipleOf(_) => "not_multiple_of",
            Check::Safe => "not_safe_integer",
        }
    }

    /// Inclusive lower / upper bound (what `clamp` moves values onto).
    fn inclusive(self) -> (Option<f64>, Option<f64>) {
        const SAFE: f64 = 9_007_199_254_740_991.0;
        match self {
            Check::Min(v) | Check::Gte(v) => (Some(v as f64), None),
            Check::Max(v) => (None, Some(v as f64)),
            Check::NonNegative => (Some(0.0), None),
            Check::NonPositive => (None, Some(0.0)),
            Check::Safe => (Some(-SAFE), Some(SAFE)),
            _ => (None, None),
        }
    }

    fn passes(self, n: f64) -> bool {
        const SAFE: f64 = 9_007_199_254_740_991.0;
        match self {
            Check::Min(v) | Check::Gte(v) => n >= v as f64,
            Check::Max(v) => n <= v as f64,
            Check::Gt(v) => n > v as f64,
            Check::Lt(v) => n < v as f64,
            Check::Positive => n > 0.0,
            Check::Negative => n < 0.0,
            Check::NonNegative => n >= 0.0,
            Check::NonPositive => n <= 0.0,
            Check::MultipleOf(0) => false,
            // Whole numbers: `%` on floats is exact.
            Check::MultipleOf(v) if n.fract() == 0.0 => n % v as f64 == 0.0,
            Check::MultipleOf(v) => {
                let q = n / v as f64;
                (q - q.round()).abs() <= 1e-9 * q.abs().max(1.0)
            }
            Check::Safe => (-SAFE..=SAFE).contains(&n),
        }
    }

    /// Exact integer form of [`passes`](Self::passes).
    fn passes_int(self, n: i128) -> bool {
        const SAFE: i128 = 9_007_199_254_740_991;
        match self {
            Check::Min(v) | Check::Gte(v) => n >= v.into(),
            Check::Max(v) => n <= v.into(),
            Check::Gt(v) => n > v.into(),
            Check::Lt(v) => n < v.into(),
            Check::Positive => n > 0,
            Check::Negative => n < 0,
            Check::NonNegative => n >= 0,
            Check::NonPositive => n <= 0,
            Check::MultipleOf(0) => false,
            Check::MultipleOf(v) => n % i128::from(v) == 0,
            Check::Safe => (-SAFE..=SAFE).contains(&n),
        }
    }
}

#[derive(Debug, Clone)]
struct Spec {
    ty: Ty,
    checks: Vec<Check>,
    clamp: bool,
    coerce: bool,
    type_error: bool,
    int_error: bool,
    /// Keys `with_messages` answers; set after `int_error` when `true`.
    messages: Vec<&'static str>,
    messages_last: bool,
    /// `one_of(...)` values (`f32()` has none).
    one_of: Option<Vec<i64>>,
}

const MESSAGE_KEYS: [&str; 7] = [
    "invalid_enum_value",
    "too_small",
    "too_big",
    "not_int",
    "not_multiple_of",
    "not_positive",
    "not_safe",
];

fn custom(key: &str) -> String {
    format!("custom {key}")
}

// ---------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------

const BOUNDS: [i64; 9] = [-300, -3, -1, 0, 1, 2, 5, 100, 255];

fn check() -> impl Strategy<Value = Check> {
    let b = || proptest::sample::select(&BOUNDS[..]);
    prop_oneof![
        b().prop_map(Check::Min),
        b().prop_map(Check::Gte),
        b().prop_map(Check::Max),
        b().prop_map(Check::Gt),
        b().prop_map(Check::Lt),
        Just(Check::Positive),
        Just(Check::Negative),
        Just(Check::NonNegative),
        Just(Check::NonPositive),
        prop_oneof![Just(0i64), Just(2), Just(3), Just(5)].prop_map(Check::MultipleOf),
        Just(Check::Safe),
    ]
}

fn spec() -> impl Strategy<Value = Spec> {
    let ty = prop_oneof![
        Just(Ty::F64),
        Just(Ty::F32),
        Just(Ty::Int),
        Just(Ty::I8),
        Just(Ty::I16),
        Just(Ty::I32),
        Just(Ty::I64),
        Just(Ty::U8),
        Just(Ty::U16),
        Just(Ty::U32),
        Just(Ty::U64),
    ];
    (
        ty,
        proptest::collection::vec(check(), 0..4),
        any::<[bool; 5]>(),
        proptest::sample::subsequence(&MESSAGE_KEYS[..], 0..=3),
        proptest::option::weighted(
            0.2,
            proptest::sample::subsequence(&[-1i64, 0, 2, 5, 255][..], 1..=3),
        ),
    )
        .prop_map(
            |(
                ty,
                checks,
                [clamp, coerce, type_error, int_error, messages_last],
                messages,
                one_of,
            )| {
                let one_of = one_of
                    .filter(|_| ty != Ty::F32)
                    .map(|vs| match ty.range() {
                        Some((lo, hi)) => vs
                            .into_iter()
                            .filter(|v| (lo..=hi).contains(&i128::from(*v)))
                            .collect(),
                        None => vs,
                    })
                    .filter(|vs: &Vec<i64>| !vs.is_empty());
                // A bound the type cannot hold cannot be passed to its builder.
                let checks = checks
                    .into_iter()
                    .filter(|c| match (c, ty.range()) {
                        (
                            Check::Min(v)
                            | Check::Gte(v)
                            | Check::Max(v)
                            | Check::Gt(v)
                            | Check::Lt(v)
                            | Check::MultipleOf(v),
                            Some((lo, hi)),
                        ) => (lo..=hi).contains(&i128::from(*v)),
                        // `f32()` has no `safe()`.
                        (Check::Safe, None) => ty != Ty::F32,
                        _ => true,
                    })
                    .collect();
                Spec {
                    ty,
                    checks,
                    clamp,
                    coerce,
                    type_error,
                    int_error,
                    messages,
                    messages_last,
                    one_of,
                }
            },
        )
}

fn input() -> impl Strategy<Value = Value> {
    let ints = prop_oneof![
        proptest::sample::select(
            &[
                // 2^53 + 1 below zero: exact only as an integer.
                -9007199254740993i64,
                -301,
                -300,
                -299,
                -129,
                -128,
                -4,
                -3,
                -2,
                -1,
                0,
                1,
                2,
                3,
                4,
                5,
                6,
                10,
                99,
                100,
                101,
                127,
                128,
                254,
                255,
                256,
                257,
                32767,
                32768,
                65535,
                65536,
                2147483647,
                2147483648,
                4294967295,
                4294967296,
                9007199254740991,
                9007199254740992,
                i64::MAX,
                i64::MIN,
            ][..]
        )
        .prop_map(Value::from),
        Just(json!(u64::MAX)),
        Just(json!(9_223_372_036_854_775_808u64)),
    ];
    let floats = proptest::sample::select(
        &[
            0.5f64, -0.5, 1.5, 2.5, 2.0, -3.0, 100.5, 255.5, 1e20, -1e20, 1e39, 1e300,
        ][..],
    )
    .prop_map(Value::from);
    let strings = proptest::sample::select(
        &[
            "5",
            "-1",
            "300",
            "2.5",
            "2.0",
            "1e3",
            "abc",
            "",
            " 7",
            "NaN",
            "inf",
            "18446744073709551616",
        ][..],
    )
    .prop_map(Value::from);
    prop_oneof![
        4 => ints,
        2 => floats,
        2 => strings,
        1 => any::<bool>().prop_map(Value::from),
        1 => Just(Value::Null),
        1 => Just(json!([1])),
    ]
}

// ---------------------------------------------------------------------------
// The schema under test
// ---------------------------------------------------------------------------

macro_rules! build_int {
    ($ctor:expr, $t:ty, $spec:expr) => {{
        let spec: &Spec = $spec;
        let mut s = $ctor;
        for c in &spec.checks {
            s = match *c {
                Check::Min(v) => s.min(v as $t),
                Check::Gte(v) => s.gte(v as $t),
                Check::Max(v) => s.max(v as $t),
                Check::Gt(v) => s.gt(v as $t),
                Check::Lt(v) => s.lt(v as $t),
                Check::Positive => s.positive(),
                Check::Negative => s.negative(),
                Check::NonNegative => s.non_negative(),
                Check::NonPositive => s.non_positive(),
                Check::MultipleOf(v) => s.multiple_of(v as $t),
                Check::Safe => s.safe(),
            };
        }
        if spec.clamp {
            s = s.clamp();
        }
        if spec.coerce {
            s = s.coerce();
        }
        if spec.type_error {
            s = s.type_error(custom("type"));
        }
        if let Some(vs) = &spec.one_of {
            let vs: Vec<$t> = vs.iter().map(|v| *v as $t).collect();
            s = s.one_of(&vs);
        }
        let msgs = messages(spec);
        if spec.messages_last {
            if spec.int_error {
                s = s.int_error(custom("int"));
            }
            s = s.with_messages(msgs);
        } else {
            s = s.with_messages(msgs);
            if spec.int_error {
                s = s.int_error(custom("int"));
            }
        }
        // A clone is the same schema.
        Erased::new(s.clone())
    }};
}

macro_rules! build_float {
    ($ctor:expr, $t:ty, $spec:expr, $safe:expr) => {{
        let spec: &Spec = $spec;
        let mut s = $ctor;
        for c in &spec.checks {
            s = match *c {
                Check::Min(v) => s.min(v as $t),
                Check::Gte(v) => s.gte(v as $t),
                Check::Max(v) => s.max(v as $t),
                Check::Gt(v) => s.gt(v as $t),
                Check::Lt(v) => s.lt(v as $t),
                Check::Positive => s.positive(),
                Check::Negative => s.negative(),
                Check::NonNegative => s.non_negative(),
                Check::NonPositive => s.non_positive(),
                Check::MultipleOf(v) => s.multiple_of(v as $t),
                Check::Safe => $safe(s),
            };
        }
        if spec.clamp {
            s = s.clamp();
        }
        if spec.coerce {
            s = s.coerce();
        }
        if spec.type_error {
            s = s.type_error(custom("type"));
        }
        Erased::new(s.with_messages(messages(spec)))
    }};
}

fn messages(spec: &Spec) -> impl Fn(&str) -> Option<String> + Send + Sync + 'static {
    let keys = spec.messages.clone();
    move |k: &str| keys.contains(&k).then(|| custom(k))
}

fn build(spec: &Spec) -> Erased {
    match spec.ty {
        Ty::F64 => {
            let base = match &spec.one_of {
                Some(vs) => vld::number().one_of(&vs.iter().map(|v| *v as f64).collect::<Vec<_>>()),
                None => vld::number(),
            };
            build_float!(base, f64, spec, |s: vld::primitives::ZNumber| s.safe())
        }
        Ty::F32 => build_float!(vld::f32(), f32, spec, |_| unreachable!(
            "f32() has no safe()"
        )),
        Ty::Int => build_int!(vld::number().int(), i64, spec),
        Ty::I8 => build_int!(vld::i8(), i8, spec),
        Ty::I16 => build_int!(vld::i16(), i16, spec),
        Ty::I32 => build_int!(vld::i32(), i32, spec),
        Ty::I64 => build_int!(vld::i64(), i64, spec),
        Ty::U8 => build_int!(vld::u8(), u8, spec),
        Ty::U16 => build_int!(vld::u16(), u16, spec),
        Ty::U32 => build_int!(vld::u32(), u32, spec),
        Ty::U64 => build_int!(vld::u64(), u64, spec),
    }
}

// ---------------------------------------------------------------------------
// The model
// ---------------------------------------------------------------------------

/// `(code, message if a custom message applies)`.
type Issue = (String, Option<String>);

fn issue(code: &str, msg: Option<String>) -> Issue {
    (code.to_string(), msg)
}

/// The number vld reads, before checks.
enum Read {
    Num(f64),
    /// An integer JSON number read by `number()`: checked exactly.
    Exact(i128),
    Int(i128),
    NotInt,
    Type,
}

fn read(spec: &Spec, v: &Value) -> Read {
    let int = spec.ty.range().is_some();
    let from_f64 = |f: f64| {
        if !int {
            Read::Num(f)
        } else if f.fract() != 0.0 {
            Read::NotInt
        } else {
            Read::Int(f as i128)
        }
    };
    match v {
        Value::Number(n) if int => match (n.as_i64(), n.as_u64()) {
            (Some(i), _) => Read::Int(i.into()),
            (_, Some(u)) => Read::Int(u.into()),
            _ => from_f64(n.as_f64().unwrap()),
        },
        Value::Number(n) if spec.ty == Ty::F64 => match (n.as_i64(), n.as_u64()) {
            (Some(i), _) => Read::Exact(i.into()),
            (_, Some(u)) => Read::Exact(u.into()),
            _ => whole(n.as_f64().unwrap()),
        },
        Value::Number(n) => Read::Num(n.as_f64().unwrap()),
        Value::String(s) if spec.coerce => match (int, s.parse::<i128>()) {
            (true, Ok(i)) => Read::Int(i),
            _ => match s.parse::<f64>() {
                Ok(f) if f.is_finite() && spec.ty == Ty::F64 => whole(f),
                Ok(f) if f.is_finite() => from_f64(f),
                _ => Read::Type,
            },
        },
        Value::Bool(b) if spec.coerce && spec.ty == Ty::F64 => Read::Exact(i128::from(*b)),
        Value::Bool(b) if spec.coerce => from_f64(if *b { 1.0 } else { 0.0 }),
        _ => Read::Type,
    }
}

/// `number()` checks a whole number exactly.
fn whole(f: f64) -> Read {
    if f.fract() == 0.0 && f.abs() < 1e38 {
        Read::Exact(f as i128)
    } else {
        Read::Num(f)
    }
}

/// The message `with_messages` / `int_error` give the `not_int` issue.
fn not_int_message(spec: &Spec) -> Option<String> {
    let wm = spec
        .messages
        .contains(&"not_int")
        .then(|| custom("not_int"));
    let ie = spec.int_error.then(|| custom("int"));
    // The later call wins; `with_messages` without a `not_int` answer keeps `int_error`.
    if spec.messages_last {
        wm.or(ie)
    } else {
        ie.or(wm)
    }
}

fn check_message(spec: &Spec, key: &str) -> Option<String> {
    spec.messages.contains(&key).then(|| custom(key))
}

/// The `one_of` issue when no allowed value matches.
fn one_of_issue(spec: &Spec, matches: impl Fn(i64) -> bool) -> Option<Issue> {
    let vs = spec.one_of.as_ref()?;
    (!vs.iter().any(|v| matches(*v))).then(|| {
        issue(
            "invalid_enum_value",
            check_message(spec, "invalid_enum_value"),
        )
    })
}

fn int_json(n: i128) -> Value {
    match i64::try_from(n) {
        Ok(i) => json!(i),
        Err(_) => json!(n as u64),
    }
}

fn predict(spec: &Spec, v: &Value) -> Result<Value, Vec<Issue>> {
    let type_msg = spec.type_error.then(|| custom("type"));
    match read(spec, v) {
        Read::Type => Err(vec![issue("invalid_type", type_msg)]),
        Read::NotInt => Err(vec![issue("not_int", not_int_message(spec))]),
        Read::Int(mut n) => {
            let (lo, hi) = spec.ty.range().unwrap();
            if spec.clamp {
                for c in &spec.checks {
                    let (l, h) = c.inclusive();
                    if let Some(l) = l.map(|l| l as i128).filter(|l| n < *l) {
                        n = l;
                    } else if let Some(h) = h.map(|h| h as i128).filter(|h| n > *h) {
                        n = h;
                    }
                }
                n = n.clamp(lo, hi);
            }
            let mut failed: Vec<Issue> = spec
                .checks
                .iter()
                .filter(|c| !c.passes_int(n))
                .map(|c| issue(c.code(), check_message(spec, c.key())))
                .collect();
            failed.extend(one_of_issue(spec, |v| i128::from(v) == n));
            if !failed.is_empty() {
                return Err(failed);
            }
            if n < lo {
                return Err(vec![issue("too_small", check_message(spec, "too_small"))]);
            }
            if n > hi {
                return Err(vec![issue("too_big", check_message(spec, "too_big"))]);
            }
            Ok(int_json(n))
        }
        Read::Exact(mut n) => {
            if spec.clamp {
                for c in &spec.checks {
                    let (l, h) = c.inclusive();
                    if let Some(l) = l.map(|l| l as i128).filter(|l| n < *l) {
                        n = l;
                    } else if let Some(h) = h.map(|h| h as i128).filter(|h| n > *h) {
                        n = h;
                    }
                }
            }
            let mut failed: Vec<Issue> = spec
                .checks
                .iter()
                .filter(|c| !c.passes_int(n))
                .map(|c| issue(c.code(), check_message(spec, c.key())))
                .collect();
            failed.extend(one_of_issue(spec, |v| i128::from(v) == n));
            if failed.is_empty() {
                Ok(json!(n as f64))
            } else {
                Err(failed)
            }
        }
        Read::Num(mut f) => {
            let f32_limit = f64::from(f32::MAX);
            if spec.clamp {
                // A bound from `non_negative` / `non_positive` / `safe` is an
                // integer: a value clamped onto it is then checked exactly.
                let mut exact = None;
                for c in &spec.checks {
                    let (l, h) = c.inclusive();
                    let int_bound =
                        matches!(c, Check::NonNegative | Check::NonPositive | Check::Safe);
                    if let Some(l) = l.filter(|l| f < *l) {
                        f = l;
                        exact = int_bound.then_some(l as i128);
                    } else if let Some(h) = h.filter(|h| f > *h) {
                        f = h;
                        exact = int_bound.then_some(h as i128);
                    }
                }
                if let (Some(n), Ty::F64) = (exact, spec.ty) {
                    return predict(
                        &Spec {
                            clamp: false,
                            ..spec.clone()
                        },
                        &json!(n as i64),
                    );
                }
                if spec.ty == Ty::F32 {
                    f = f.clamp(-f32_limit, f32_limit);
                }
            }
            if spec.ty == Ty::F32 {
                let out = f as f32;
                if out == f32::NEG_INFINITY {
                    return Err(vec![issue("too_small", check_message(spec, "too_small"))]);
                }
                if out == f32::INFINITY {
                    return Err(vec![issue("too_big", check_message(spec, "too_big"))]);
                }
                f = out.to_string().parse().unwrap();
            }
            let mut failed: Vec<Issue> = spec
                .checks
                .iter()
                .filter(|c| !c.passes(f))
                .map(|c| issue(c.code(), check_message(spec, c.key())))
                .collect();
            failed.extend(one_of_issue(spec, |v| v as f64 == f));
            if !failed.is_empty() {
                return Err(failed);
            }
            Ok(if spec.ty == Ty::F32 {
                json!(f as f32)
            } else {
                json!(f)
            })
        }
    }
}

fn check_case(spec: &Spec, v: &Value) -> Result<(), TestCaseError> {
    let got = build(spec).parse_value(v);
    match (&got, predict(spec, v)) {
        (Ok(g), Ok(w)) => prop_assert_eq!(g, &w, "{:?} on {}", spec, v),
        (Err(e), Err(w)) => {
            let codes: Vec<&str> = e.issues.iter().map(|i| i.code.key()).collect();
            let want: Vec<&str> = w.iter().map(|(c, _)| c.as_str()).collect();
            prop_assert_eq!(&codes, &want, "{:?} on {}: {}", spec, v, e);
            for (i, (_, msg)) in e.issues.iter().zip(&w) {
                if let Some(msg) = msg {
                    prop_assert_eq!(&i.message, msg, "{:?} on {}", spec, v);
                }
                prop_assert_eq!(path_of(i), "");
            }
        }
        (g, w) => prop_assert!(false, "{:?} on {}: got {:?}, want {:?}", spec, v, g, w),
    }
    Ok(())
}

#[test]
fn numbers_agree_with_the_model() {
    let mut runner = TestRunner::new(Config {
        cases: 4000,
        ..Config::default()
    });
    runner
        .run(&(spec(), input()), |(s, v)| check_case(&s, &v))
        .unwrap();
}

/// Given a number schema with checks, when its JSON Schema is emitted, then a
/// JSON Schema validator accepts exactly the numbers vld accepts (no clamp,
/// no coercion; `int()` inputs stay within `i64`).
#[cfg(feature = "openapi")]
#[test]
fn number_json_schema_agrees() {
    use vld::openapi::JsonSchema;
    let mut runner = TestRunner::new(Config {
        cases: 2000,
        ..Config::default()
    });
    let strat = (spec(), input()).prop_filter("exact", |(s, v)| {
        !s.clamp
            && !s.coerce
            // vld f32 rounds before comparing; the schema compares the input.
            && s.ty != Ty::F32
            // `multiple_of(0)` never matches; JSON Schema cannot say so (`multipleOf > 0`).
            && !s.checks.iter().any(|c| matches!(c, Check::MultipleOf(0)))
            && !(s.ty == Ty::Int && v.as_f64().is_some_and(|f| f.fract() == 0.0) && v.as_i64().is_none())
    });
    runner
        .run(&strat, |(s, v)| {
            let schema = build(&s).json_schema();
            let ok = build(&s).parse_value(&v).is_ok();
            let validator = jsonschema::draft202012::new(&schema).unwrap();
            prop_assert_eq!(validator.is_valid(&v), ok, "{:?}: {} on {}", s, schema, v);
            Ok(())
        })
        .unwrap();
}

proptest! {
    /// Given the presets `percent()` (`0..=100`) and `ratio()` (`0..=1`)
    /// When a number around and beyond both ends (quarter steps) is parsed
    /// Then it is returned inside the range, `too_small` (minimum 0) below and `too_big` (maximum
    /// 100 / 1) above; the JSON Schema is `number` with `minimum` / `maximum`.
    #[test]
    fn percent_and_ratio_presets(q in -8i32..=420, ratio in any::<bool>()) {
        let (schema, top) = if ratio { (vld::ratio(), 1.0) } else { (vld::percent(), 100.0) };
        let v = f64::from(q) / if ratio { 400.0 } else { 4.0 };
        let want = if v < 0.0 {
            Some("too_small")
        } else if v > top {
            Some("too_big")
        } else {
            None
        };
        match (schema.parse_value(&json!(v)), want) {
            (Ok(out), None) => prop_assert_eq!(out, v),
            (Err(err), Some(code)) => prop_assert_eq!(issues(&err), vec![(String::new(), code.to_string())]),
            (got, want) => prop_assert!(false, "{v}: got {got:?}, want {want:?}"),
        }
        #[cfg(feature = "openapi")]
        {
            use vld::openapi::JsonSchema;
            prop_assert_eq!(schema.json_schema(), json!({"type": "number", "minimum": 0.0, "maximum": top}));
        }
    }
}
