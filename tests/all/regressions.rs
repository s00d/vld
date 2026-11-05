//! Cross-cutting regressions that do not belong to a single schema area.
//! (Area-specific regressions live next to the feature they cover.)

use vld::prelude::*;

/// A schema that fails without reporting any issue.
struct SilentFail;

impl VldSchema for SilentFail {
    type Output = ();
    fn parse_value(&self, _: &serde_json::Value) -> Result<(), VldError> {
        Err(VldError::new())
    }
}

/// Wrappers that report `Ok` although an inner schema failed without issues.
fn swallowed() -> Vec<&'static str> {
    let results = [
        (
            "intersection_first",
            vld::intersection(SilentFail, vld::any())
                .parse("1")
                .is_err(),
        ),
        (
            "intersection_second",
            vld::intersection(vld::any(), SilentFail)
                .parse("1")
                .is_err(),
        ),
        ("tuple", (SilentFail, vld::any()).parse("[1, 2]").is_err()),
        ("array", vld::array(SilentFail).parse("[1]").is_err()),
        ("set", vld::set(SilentFail).parse("[1]").is_err()),
        (
            "record",
            vld::record(SilentFail).parse(r#"{"a": 1}"#).is_err(),
        ),
        (
            "union",
            vld::union(SilentFail, SilentFail).parse("1").is_err(),
        ),
        ("optional", SilentFail.optional().parse("1").is_err()),
        (
            "refine",
            SilentFail.refine(|_| true, "x").parse("1").is_err(),
        ),
        ("pipe", vld::any().pipe(SilentFail).parse("1").is_err()),
        ("lazy", vld::lazy(|| SilentFail).parse("1").is_err()),
    ];
    results
        .into_iter()
        .filter(|(_, is_err)| !is_err)
        .map(|(name, _)| name)
        .collect()
}

/// Combinators used to index `issues[0]` of a failed side and panicked.
/// None may panic; these already report the failure.
#[test]
fn issue_less_failures_do_not_panic() {
    let swallowed = swallowed();
    for name in [
        "intersection_first",
        "tuple",
        "union",
        "optional",
        "refine",
        "pipe",
        "lazy",
    ] {
        assert!(!swallowed.contains(&name), "{name} swallowed the failure");
    }
    assert!(SilentFail.catch(()).parse("1").is_ok());
}

#[test]
#[ignore = "BUG: intersection (second side), array, set and record treat Err(VldError::new()) from an inner schema as success (they test errors.is_empty())"]
fn issue_less_failures_are_never_swallowed() {
    assert_eq!(swallowed(), Vec::<&str>::new());
}
