use crate::common::{codes, parse_err, paths};
use serde_json::json;
use vld::error::{IssueCode, SizeOrigin};
use vld::prelude::*;

#[test]
fn length_bounds() {
    let a = vld::array(vld::string()).min_len(1).max_len(3);
    assert_eq!(
        a.parse_value(&json!(["a", "b"])).unwrap(),
        vec!["a".to_string(), "b".to_string()]
    );
    assert_issue!(
        parse_err(&a, json!([])),
        "",
        IssueCode::TooSmall { origin: SizeOrigin::Array, minimum, .. } if minimum == &json!(1)
    );
    assert_issue!(
        parse_err(&a, json!(["a", "b", "c", "d"])),
        "",
        IssueCode::TooBig { origin: SizeOrigin::Array, maximum, .. } if maximum == &json!(3)
    );
    let exact = vld::array(vld::any()).len(2);
    assert!(exact.parse_value(&json!([1, 2])).is_ok());
    assert_issue!(
        parse_err(&exact, json!([1])),
        "",
        IssueCode::InvalidLength {
            origin: SizeOrigin::Array,
            expected: 2
        }
    );
    let non_empty = vld::array(vld::any()).non_empty();
    assert_issue!(
        parse_err(&non_empty, json!([])),
        "",
        IssueCode::TooSmall { .. }
    );
    assert_issue!(parse_err(&a, json!("a")), "", IssueCode::InvalidType { expected, .. } if expected == "array");
}

#[test]
fn every_bad_element_is_reported_with_its_index() {
    let a = vld::array(vld::string().min(2));
    let err = parse_err(&a, json!(["ok", "x", "fine", "y", 3]));
    assert_eq!(paths(&err), ["[1]", "[3]", "[4]"]);
    assert_eq!(codes(&err), ["too_small", "too_small", "invalid_type"]);
}

#[test]
fn nested_paths() {
    let a = vld::array(vld::array(vld::number().int()));
    let err = parse_err(&a, json!([[1], [2, "x"]]));
    assert_eq!(paths(&err), ["[1][1]"]);
}

#[test]
fn contains_counts_and_unique() {
    let a = vld::array(vld::number().int())
        .contains(json!(2))
        .min_contains(2)
        .max_contains(3);
    assert!(a.parse_value(&json!([2, 1, 2])).is_ok());
    assert_issue!(
        parse_err(&vld::array(vld::any()).contains(json!(2)), json!([1, 3])),
        "",
        IssueCode::MissingContains
    );
    assert_issue!(
        parse_err(&a, json!([1, 3])),
        "",
        IssueCode::TooSmall {
            origin: SizeOrigin::Contains,
            ..
        }
    );
    assert_issue!(
        parse_err(&a, json!([1, 2])),
        "",
        IssueCode::TooSmall {
            origin: SizeOrigin::Contains,
            ..
        }
    );
    assert_issue!(
        parse_err(&a, json!([2, 2, 2, 2])),
        "",
        IssueCode::TooBig {
            origin: SizeOrigin::Contains,
            ..
        }
    );
    // min_contains(0) makes `contains` optional.
    let opt = vld::array(vld::any()).contains(1).min_contains(0);
    assert!(opt.parse("[]").is_ok());
}

/// Duplicates are flagged at each repeat; numbers compare by value and
/// objects ignore key order.
#[test]
fn unique_paths_and_equality() {
    let u = vld::array(vld::any()).unique();
    assert!(u.parse_value(&json!([1, 2, "1"])).is_ok());
    let err = u.parse("[1, 2, 1.0, 2, 3]").unwrap_err();
    assert_eq!(paths(&err), ["[2]", "[3]"]);
    assert!(err.issues.iter().all(|i| i.code == IssueCode::NotUnique));
    assert!(u.parse(r#"[{"a":1,"b":2},{"b":2,"a":1.0}]"#).is_err());
}
