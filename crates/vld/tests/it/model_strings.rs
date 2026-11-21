//! Model test: string schemas with random checks, transforms and messages.
//!
//! proptest picks transforms (`trim`, `to_lowercase`, `to_uppercase`, in
//! order), checks (`min`, `max`, `len`, `non_empty`, `starts_with`,
//! `ends_with`, `contains`, the `ascii` / `alpha` / `alphanumeric` /
//! `lowercase` / `uppercase` formats, the password rules `require_lowercase` /
//! `require_uppercase` / `require_digit` / `require_symbol` / `max_repeat`, a
//! closure rule, a pattern), per-check `*_msg` messages, `with_messages`,
//! `type_error`, `coerce` / `coerce_bools`, `redact()`, and an input mixing ASCII, case-changing letters (`ß`,
//! `İ`), non-ASCII whitespace, numbers, booleans and `null`. The model
//! predicts the output, or every issue code in order with the message a custom
//! message gives it.

use crate::common::*;

use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use serde_json::{Value, json};
use vld::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Transform {
    Trim,
    Lower,
    Upper,
    Truncate(usize),
}

#[derive(Debug, Clone, PartialEq)]
enum Check {
    Min(usize),
    Max(usize),
    MinBytes(usize),
    MaxBytes(usize),
    Len(usize),
    NonEmpty,
    StartsWith(&'static str),
    EndsWith(&'static str),
    Contains(&'static str),
    Ascii,
    Alpha,
    Alphanumeric,
    Lowercase,
    Uppercase,
    /// `rules::from_fn("no_digits", ..)`, with or without its own message.
    NoDigits(bool),
    ReqLower,
    ReqUpper,
    ReqDigit,
    ReqSymbol,
    MaxRepeat(usize),
    #[cfg(feature = "regex")]
    /// `pattern("^[a-z]")`.
    StartsLower,
}

impl Check {
    fn key(&self) -> &'static str {
        match self {
            Check::Min(_) | Check::MinBytes(_) => "too_small",
            Check::Max(_) | Check::MaxBytes(_) => "too_big",
            Check::Len(_) => "invalid_length",
            Check::NonEmpty => "non_empty",
            Check::StartsWith(_) => "invalid_starts_with",
            Check::EndsWith(_) => "invalid_ends_with",
            Check::Contains(_) => "invalid_contains",
            Check::Ascii => "invalid_ascii",
            Check::Alpha => "invalid_alpha",
            Check::Alphanumeric => "invalid_alphanumeric",
            Check::Lowercase => "invalid_lowercase",
            Check::Uppercase => "invalid_uppercase",
            Check::NoDigits(_) => "no_digits",
            Check::ReqLower => "invalid_password_lowercase",
            Check::ReqUpper => "invalid_password_uppercase",
            Check::ReqDigit => "invalid_password_digit",
            Check::ReqSymbol => "invalid_password_symbol",
            Check::MaxRepeat(_) => "invalid_password_repeat",
            #[cfg(feature = "regex")]
            Check::StartsLower => "invalid_regex",
        }
    }

    fn code(&self) -> &'static str {
        match self {
            Check::Min(_) | Check::MinBytes(_) | Check::NonEmpty => "too_small",
            Check::Max(_) | Check::MaxBytes(_) => "too_big",
            Check::Len(_) => "invalid_length",
            Check::NoDigits(_) => "no_digits",
            _ => "invalid_string",
        }
    }

    fn passes(&self, s: &str) -> bool {
        let n = s.chars().count();
        match self {
            Check::Min(m) => n >= *m,
            Check::Max(m) => n <= *m,
            Check::MinBytes(m) => s.len() >= *m,
            Check::MaxBytes(m) => s.len() <= *m,
            Check::Len(m) => n == *m,
            Check::NonEmpty => !s.is_empty(),
            Check::StartsWith(p) => s.starts_with(p),
            Check::EndsWith(p) => s.ends_with(p),
            Check::Contains(p) => s.contains(p),
            Check::Ascii => s.is_ascii(),
            Check::Alpha => s.chars().all(|c| c.is_ascii_alphabetic()),
            Check::Alphanumeric => s.chars().all(|c| c.is_ascii_alphanumeric()),
            Check::Lowercase => s == s.to_lowercase(),
            Check::Uppercase => s == s.to_uppercase(),
            Check::NoDigits(_) => !s.chars().any(|c| c.is_ascii_digit()),
            Check::ReqLower => s.chars().any(char::is_lowercase),
            Check::ReqUpper => s.chars().any(char::is_uppercase),
            Check::ReqDigit => s.chars().any(|c| c.is_ascii_digit()),
            Check::ReqSymbol => s
                .chars()
                .any(|c| !c.is_alphanumeric() && !c.is_whitespace()),
            // The longest run of one character.
            Check::MaxRepeat(m) => {
                let chars: Vec<char> = s.chars().collect();
                chars
                    .chunk_by(|a, b| a == b)
                    .map(<[char]>::len)
                    .max()
                    .unwrap_or(0)
                    <= *m
            }
            #[cfg(feature = "regex")]
            Check::StartsLower => s.chars().next().is_some_and(|c| c.is_ascii_lowercase()),
        }
    }

    /// Expressible in JSON Schema exactly (no custom format, no rule).
    #[cfg_attr(not(feature = "openapi"), allow(dead_code))]
    fn in_json_schema(&self) -> bool {
        !matches!(
            self,
            Check::Ascii
                | Check::Alpha
                | Check::Alphanumeric
                | Check::Lowercase
                | Check::Uppercase
                | Check::NoDigits(_)
                | Check::ReqLower
                | Check::ReqUpper
                | Check::ReqDigit
                | Check::ReqSymbol
                | Check::MaxRepeat(_)
                | Check::MinBytes(_)
                | Check::MaxBytes(_)
        )
    }
}

#[derive(Debug, Clone)]
struct Spec {
    transforms: Vec<Transform>,
    /// `(check, per-check message?)`.
    checks: Vec<(Check, bool)>,
    messages: Vec<&'static str>,
    type_error: bool,
    /// `None`, `coerce()`, or `coerce_bools("1", "0")`.
    coerce: Option<bool>,
    /// Wrapped in `.redact()`.
    redact: bool,
}

fn custom(what: &str) -> String {
    format!("custom {what}")
}

// ---------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------

fn check() -> impl Strategy<Value = Check> {
    let n = || 0usize..5;
    let frag = || proptest::sample::select(&["a", "A", "é", "ab", " "][..]);
    #[cfg_attr(not(feature = "regex"), allow(unused_mut))]
    let mut options: Vec<BoxedStrategy<Check>> = vec![
        n().prop_map(Check::Min).boxed(),
        n().prop_map(Check::Max).boxed(),
        n().prop_map(Check::Len).boxed(),
        (0usize..9).prop_map(Check::MinBytes).boxed(),
        (0usize..9).prop_map(Check::MaxBytes).boxed(),
        Just(Check::NonEmpty).boxed(),
        frag().prop_map(Check::StartsWith).boxed(),
        frag().prop_map(Check::EndsWith).boxed(),
        frag().prop_map(Check::Contains).boxed(),
        Just(Check::Ascii).boxed(),
        Just(Check::Alpha).boxed(),
        Just(Check::Alphanumeric).boxed(),
        Just(Check::Lowercase).boxed(),
        Just(Check::Uppercase).boxed(),
        any::<bool>().prop_map(Check::NoDigits).boxed(),
        Just(Check::ReqLower).boxed(),
        Just(Check::ReqUpper).boxed(),
        Just(Check::ReqDigit).boxed(),
        Just(Check::ReqSymbol).boxed(),
        (1usize..3).prop_map(Check::MaxRepeat).boxed(),
    ];
    #[cfg(feature = "regex")]
    options.push(Just(Check::StartsLower).boxed());
    proptest::strategy::Union::new(options)
}

const MESSAGE_KEYS: [&str; 10] = [
    "too_small",
    "too_big",
    "invalid_length",
    "non_empty",
    "invalid_contains",
    "invalid_ascii",
    "no_digits",
    "invalid_regex",
    "invalid_password_symbol",
    "invalid_password_repeat",
];

fn spec() -> impl Strategy<Value = Spec> {
    let transform = prop_oneof![
        Just(Transform::Trim),
        Just(Transform::Lower),
        Just(Transform::Upper),
        (0usize..4).prop_map(Transform::Truncate),
    ];
    (
        proptest::collection::vec(transform, 0..3),
        proptest::collection::vec((check(), any::<bool>()), 0..4),
        proptest::sample::subsequence(&MESSAGE_KEYS[..], 0..=3),
        any::<bool>(),
        proptest::option::of(any::<bool>()),
        any::<bool>(),
    )
        .prop_map(
            |(transforms, checks, messages, type_error, coerce, redact)| Spec {
                transforms,
                checks,
                messages,
                type_error,
                coerce,
                redact,
            },
        )
}

fn input() -> impl Strategy<Value = Value> {
    let piece = proptest::sample::select(
        &[
            "a", "b", "A", "Z", "é", "É", "ß", "İ", " ", "\u{3000}", "1", "x", "_", "😀", "aa", "€",
        ][..],
    );
    prop_oneof![
        6 => proptest::collection::vec(piece, 0..6).prop_map(|p| Value::from(p.concat())),
        1 => prop_oneof![Just(json!(5)), Just(json!(-1.5)), Just(json!(u64::MAX))],
        1 => any::<bool>().prop_map(Value::from),
        1 => Just(Value::Null),
        1 => Just(json!(["a"])),
    ]
}

// ---------------------------------------------------------------------------
// The schema under test
// ---------------------------------------------------------------------------

fn build(spec: &Spec) -> Erased {
    let mut s = vld::string();
    for t in &spec.transforms {
        s = match t {
            Transform::Trim => s.trim(),
            Transform::Lower => s.to_lowercase(),
            Transform::Upper => s.to_uppercase(),
            Transform::Truncate(n) => s.truncate(*n),
        };
    }
    for (c, with_msg) in &spec.checks {
        let msg = custom(c.key());
        s = match (c.clone(), with_msg) {
            (Check::Min(n), false) => s.min(n),
            (Check::Min(n), true) => s.min_msg(n, msg),
            (Check::Max(n), false) => s.max(n),
            (Check::Max(n), true) => s.max_msg(n, msg),
            // No `*_msg` form: the per-check message falls to `with_messages`.
            (Check::MinBytes(n), _) => s.min_bytes(n),
            (Check::MaxBytes(n), _) => s.max_bytes(n),
            (Check::Len(n), false) => s.len(n),
            (Check::Len(n), true) => s.len_msg(n, msg),
            (Check::NonEmpty, false) => s.non_empty(),
            (Check::NonEmpty, true) => s.non_empty_msg(msg),
            (Check::StartsWith(p), false) => s.starts_with(p),
            (Check::StartsWith(p), true) => s.starts_with_msg(p, msg),
            (Check::EndsWith(p), false) => s.ends_with(p),
            (Check::EndsWith(p), true) => s.ends_with_msg(p, msg),
            (Check::Contains(p), false) => s.contains(p),
            (Check::Contains(p), true) => s.contains_msg(p, msg),
            (Check::Ascii, false) => s.ascii(),
            (Check::Ascii, true) => s.ascii_msg(msg),
            (Check::Alpha, false) => s.alpha(),
            (Check::Alpha, true) => s.alpha_msg(msg),
            (Check::Alphanumeric, false) => s.alphanumeric(),
            (Check::Alphanumeric, true) => s.alphanumeric_msg(msg),
            (Check::Lowercase, false) => s.lowercase(),
            (Check::Lowercase, true) => s.lowercase_msg(msg),
            (Check::Uppercase, false) => s.uppercase(),
            (Check::Uppercase, true) => s.uppercase_msg(msg),
            (Check::NoDigits(own), with_msg) => {
                let mut r = vld::rules::from_fn("no_digits", |s: &str| {
                    !s.chars().any(|c| c.is_ascii_digit())
                });
                if own {
                    r = r.message(custom("rule"));
                }
                if *with_msg {
                    s.rule_msg(r, msg)
                } else {
                    s.rule(r)
                }
            }
            (Check::ReqLower, false) => s.require_lowercase(),
            (Check::ReqLower, true) => s.require_lowercase_msg(msg),
            (Check::ReqUpper, false) => s.require_uppercase(),
            (Check::ReqUpper, true) => s.require_uppercase_msg(msg),
            (Check::ReqDigit, false) => s.require_digit(),
            (Check::ReqDigit, true) => s.require_digit_msg(msg),
            (Check::ReqSymbol, false) => s.require_symbol(),
            (Check::ReqSymbol, true) => s.require_symbol_msg(msg),
            (Check::MaxRepeat(n), false) => s.max_repeat(n),
            (Check::MaxRepeat(n), true) => s.max_repeat_msg(n, msg),
            #[cfg(feature = "regex")]
            (Check::StartsLower, false) => s.pattern("^[a-z]"),
            #[cfg(feature = "regex")]
            (Check::StartsLower, true) => s.pattern_msg("^[a-z]", msg),
        };
    }
    let keys = spec.messages.clone();
    s = s.with_messages(move |k| keys.contains(&k).then(|| custom(&format!("bulk {k}"))));
    if spec.type_error {
        s = s.type_error(custom("type"));
    }
    s = match spec.coerce {
        None => s,
        Some(false) => s.coerce(),
        Some(true) => s.coerce_bools("1", "0"),
    };
    if spec.redact {
        Erased::new(s.redact())
    } else {
        Erased::new(s)
    }
}

// ---------------------------------------------------------------------------
// The model
// ---------------------------------------------------------------------------

type Issue = (String, Option<String>);

fn predict(spec: &Spec, v: &Value) -> Result<Value, Vec<Issue>> {
    let mut s = match (v, spec.coerce) {
        (Value::String(s), _) => s.clone(),
        (Value::Number(n), Some(_)) => n.to_string(),
        (Value::Bool(b), Some(false)) => b.to_string(),
        (Value::Bool(b), Some(true)) => if *b { "1" } else { "0" }.to_string(),
        _ => {
            return Err(vec![(
                "invalid_type".into(),
                spec.type_error.then(|| custom("type")),
            )]);
        }
    };
    for t in &spec.transforms {
        s = match t {
            Transform::Trim => s.trim().to_string(),
            Transform::Lower => s.to_lowercase(),
            Transform::Upper => s.to_uppercase(),
            Transform::Truncate(n) => s.chars().take(*n).collect(),
        };
    }
    let failed: Vec<Issue> = spec
        .checks
        .iter()
        .filter(|(c, _)| !c.passes(&s))
        .map(|(c, with_msg)| {
            // Per-check message, then `with_messages`, then the rule's own.
            let has_msg_form = !matches!(c, Check::MinBytes(_) | Check::MaxBytes(_));
            let msg = (*with_msg && has_msg_form)
                .then(|| custom(c.key()))
                .or_else(|| {
                    spec.messages
                        .contains(&c.key())
                        .then(|| custom(&format!("bulk {}", c.key())))
                })
                .or_else(|| matches!(c, Check::NoDigits(true)).then(|| custom("rule")));
            (c.code().to_string(), msg)
        })
        .collect();
    if failed.is_empty() {
        Ok(json!(s))
    } else {
        Err(failed)
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
            }
            // A redacted schema reports the marker as received by every issue.
            if spec.redact {
                for i in &e.issues {
                    prop_assert_eq!(i.received.as_ref(), Some(&json!("[redacted]")));
                }
                if let Value::String(input) = v {
                    if input.chars().count() >= 4 {
                        prop_assert!(!e.to_string().contains(input.as_str()), "{}", e);
                    }
                }
            // The transformed string is reported as received by every check.
            } else if let Ok(shown) = predict(
                &Spec {
                    checks: vec![],
                    ..spec.clone()
                },
                v,
            ) {
                for i in &e.issues {
                    prop_assert_eq!(i.received.as_ref(), Some(&shown));
                }
            }
        }
        (g, w) => prop_assert!(false, "{:?} on {}: got {:?}, want {:?}", spec, v, g, w),
    }
    Ok(())
}

#[test]
fn strings_agree_with_the_model() {
    let mut runner = TestRunner::new(Config {
        cases: 4000,
        ..Config::default()
    });
    runner
        .run(&(spec(), input()), |(s, v)| check_case(&s, &v))
        .unwrap();
}

/// Given a string schema with length and substring checks, when its JSON
/// Schema is emitted, then a JSON Schema validator accepts exactly the strings
/// vld accepts (no transforms, coercion, custom formats or rules).
#[cfg(feature = "openapi")]
#[test]
fn string_json_schema_agrees() {
    use vld::openapi::JsonSchema;
    let mut runner = TestRunner::new(Config {
        cases: 2000,
        ..Config::default()
    });
    let strat = (spec(), input()).prop_filter("exact", |(s, _)| {
        s.transforms.is_empty()
            && s.coerce.is_none()
            && s.checks.iter().all(|(c, _)| c.in_json_schema())
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
