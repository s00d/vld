//! Stories: single fields checked before a `MERGE` (`validate_value`,
//! `validate_fields!`).

use crate::common::*;
use serde_json::json;
use vld_surrealdb::prelude::*;

/// Given field schemas (string length, integer range),
/// when single JSON values are checked with `validate_value`,
/// then values inside the bounds pass and the others report the root path with
/// `too_small` / `too_big` / `invalid_type` / `not_int`.
#[test]
fn single_values_are_checked_against_a_schema_instance() {
    let name = vld::string().min(1).max(100);
    for ok in ["Hello", "H", &"x".repeat(100)] {
        validate_value(&name, &json!(ok)).unwrap();
    }
    assert_eq!(
        issues(validate_value(&name, &json!("x".repeat(101)))),
        pairs(&[("", "too_big")])
    );
    assert_eq!(
        issues(validate_value(&name, &json!(""))),
        pairs(&[("", "too_small")])
    );
    assert_eq!(
        issues(validate_value(&name, &json!(5))),
        pairs(&[("", "invalid_type")])
    );

    let qty = vld::number().int().min(0).max(100);
    for ok in [0, 42, 100] {
        validate_value(&qty, &json!(ok)).unwrap();
    }
    assert_eq!(
        issues(validate_value(&qty, &json!(1.5))),
        pairs(&[("", "not_int")])
    );
    assert_eq!(
        issues(validate_value(&qty, &json!(101))),
        pairs(&[("", "too_big")])
    );
}

/// Given local variables about to be merged into a record,
/// when they are checked together with `validate_fields!`,
/// then valid ones pass, and invalid ones keep their real issue codes and
/// messages under a path starting with the variable name (nested issues keep
/// their inner path: `.tags[1]`).
#[test]
fn fields_are_checked_together_before_merge() {
    let name = "Alice";
    let email = "alice@example.com";
    validate_fields! {
        name => vld::string().min(1).max(100),
        email => vld::string().email(),
    }
    .unwrap();

    let name = "";
    let email = "not-email";
    let tags = vec!["ok", ""];
    let err = validate_fields! {
        name => vld::string().min(3),
        email => vld::string().email(),
        tags => vld::array(vld::string().min(1)),
    }
    .unwrap_err();
    assert_eq!(
        issues_of(&err),
        pairs(&[
            (".name", "too_small"),
            (".email", "invalid_string"),
            (".tags[1]", "too_small")
        ])
    );
    let VldSurrealError::Validation(e) = &err else {
        unreachable!()
    };
    let direct = vld::string().min(3).parse_value(&json!("")).unwrap_err();
    assert_eq!(e.issues[0].message, direct.issues[0].message);
    let resp = VldSurrealResponse::from_error(&err);
    let fields: Vec<&str> = resp.fields.iter().map(|f| f.field.as_str()).collect();
    assert_eq!(fields, ["name", "email", "tags[1]"]);
}

/// Given a field that cannot be serialized between valid / invalid fields,
/// when `validate_fields!` runs,
/// then the result is a `Serialization` error naming the first such field,
/// whatever comes before or after it.
#[test]
fn unserializable_field_stops_the_check() {
    let bad = Unserializable;
    let later = Unserializable;
    let name = "";
    let err = validate_fields! { bad => vld::any() }.unwrap_err();
    assert!(
        matches!(&err, VldSurrealError::Serialization(m) if m == "field 'bad': nope"),
        "{err:?}"
    );

    let ok = "fine";
    let err = validate_fields! {
        ok => vld::string(),
        bad => vld::any(),
        name => vld::string().min(1),
        later => vld::any(),
    }
    .unwrap_err();
    assert!(
        matches!(&err, VldSurrealError::Serialization(m) if m == "field 'bad': nope"),
        "{err:?}"
    );

    let err = validate_fields! {
        name => vld::string().min(1),
        bad => vld::any(),
    }
    .unwrap_err();
    assert!(matches!(err, VldSurrealError::Serialization(_)), "{err:?}");
}
