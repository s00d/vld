//! `vld::format`: prettify / flatten / treeify.

use serde_json::json;
use vld::prelude::*;

fn sample_error() -> VldError {
    vld::object()
        .field("name", vld::string().min(5))
        .field("age", vld::number().positive())
        .field("user", vld::object().field("email", vld::string().email()))
        .parse_value(&json!({"name": "ab", "age": -1, "user": {"email": "x"}}))
        .unwrap_err()
}

#[test]
fn prettify_lists_every_issue_with_its_path() {
    let pretty = vld::format::prettify_error(&sample_error());
    let lines: Vec<&str> = pretty.lines().collect();
    assert_eq!(lines.len(), 6, "{pretty}");
    assert!(
        lines[0].starts_with("✖ String must be at least 5 characters"),
        "{pretty}"
    );
    assert!(lines[1].starts_with("  → at .name"), "{pretty}");
    assert!(lines[3].starts_with("  → at .age"), "{pretty}");
    assert!(lines[5].starts_with("  → at .user.email"), "{pretty}");
}

#[test]
fn flatten_groups_by_top_level_field() {
    let flat = vld::format::flatten_error(&sample_error());
    assert_eq!(flat.field_errors["name"].len(), 1);
    assert_eq!(flat.field_errors["age"].len(), 1);
    assert_eq!(flat.field_errors["user"].len(), 1);
    assert!(flat.form_errors.is_empty());

    let root = vld::string().parse("1").unwrap_err();
    let flat = vld::format::flatten_error(&root);
    assert_eq!(flat.form_errors, ["Expected string, received number"]);
    assert!(flat.field_errors.is_empty());
}

#[test]
fn treeify_mirrors_the_shape() {
    let tree = vld::format::treeify_error(&sample_error());
    assert_eq!(tree.properties["name"].errors.len(), 1);
    assert_eq!(tree.properties["user"].properties["email"].errors.len(), 1);
    assert!(tree.properties["user"].errors.is_empty());
}
