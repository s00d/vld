use crate::common::{parse_err, paths};
use serde_json::json;
use vld::error::{IssueCode, SizeOrigin};
use vld::prelude::*;

#[test]
fn deduplicates() {
    let s = vld::set(vld::string());
    let out = s.parse(r#"["a", "b", "a"]"#).unwrap();
    assert_eq!(out.len(), 2);
    assert!(out.contains("a") && out.contains("b"));
    assert!(s.parse("[]").unwrap().is_empty());
    assert_issue!(parse_err(&s, json!("a")), "", IssueCode::InvalidType { .. });
}

/// Size bounds count unique elements.
#[test]
fn size_bounds_count_unique_elements() {
    let s = vld::set(vld::number().int()).min_size(2).max_size(2);
    assert!(s.parse("[1, 2]").is_ok());
    assert!(s.parse("[1, 2, 2, 1]").is_ok());
    assert_issue!(
        s.parse("[1, 1]"),
        "",
        IssueCode::TooSmall { origin: SizeOrigin::Set, minimum, .. } if minimum == &json!(2)
    );
    assert_issue!(
        s.parse("[1, 2, 3]"),
        "",
        IssueCode::TooBig { origin: SizeOrigin::Set, maximum, .. } if maximum == &json!(2)
    );
}

#[test]
fn element_errors_have_index_paths() {
    let s = vld::set(vld::string().non_empty());
    let err = s.parse(r#"["a", "", "b", 1]"#).unwrap_err();
    assert_eq!(paths(&err), ["[1]", "[3]"]);
}
