use crate::common::{parse_err, paths};
use serde_json::json;
use vld::error::IssueCode;
use vld::prelude::*;

fn animal() -> ZDiscriminatedUnion {
    vld::discriminated_union("type")
        .variant_str(
            "dog",
            vld::object()
                .field("type", vld::literal("dog"))
                .field("bark", vld::boolean()),
        )
        .variant_str(
            "cat",
            vld::object()
                .field("type", vld::literal("cat"))
                .field("lives", vld::number().int()),
        )
}

#[test]
fn routes_by_discriminator() {
    assert_eq!(
        animal().parse(r#"{"type":"dog","bark":true}"#).unwrap()["bark"],
        json!(true)
    );
    assert_eq!(
        animal().parse(r#"{"type":"cat","lives":9}"#).unwrap()["lives"],
        json!(9)
    );
    // Only the chosen variant's errors are reported.
    let err = animal().parse(r#"{"type":"cat","lives":"x"}"#).unwrap_err();
    assert_eq!(paths(&err), [".lives"]);
}

#[test]
fn bad_or_missing_discriminator_points_at_it() {
    let err = animal().parse(r#"{"type":"fish"}"#).unwrap_err();
    assert_issue!(
        &err,
        ".type",
        IssueCode::InvalidDiscriminator { options } if options == &[json!("dog"), json!("cat")]
    );
    assert_issue!(
        animal().parse(r#"{"name":"x"}"#),
        ".type",
        IssueCode::MissingField
    );
    assert_issue!(
        parse_err(&animal(), json!("hello")),
        "",
        IssueCode::InvalidType { .. }
    );
}

#[test]
fn variants_in_insertion_order() {
    let keys: Vec<serde_json::Value> = animal().variants().map(|(k, _)| k.clone()).collect();
    assert_eq!(keys, [json!("dog"), json!("cat")]);
    let du = animal();
    let (_, cat) = du.variants().nth(1).unwrap();
    assert!(cat.dyn_parse(&json!({"type": "cat", "lives": 1})).is_ok());
    assert!(cat.dyn_parse(&json!({"type": "cat"})).is_err());
}
