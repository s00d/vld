//! `IssueCode`, `ValidationIssue`, `VldError` constructors and `IssueBuilder`.

use crate::common::{codes, path_of, paths};
use serde_json::json;
use vld::error::{IssueCode, PathSegment, SizeOrigin, ValidationIssue, VldError};
use vld::prelude::*;

fn too_small() -> IssueCode {
    IssueCode::TooSmall {
        origin: SizeOrigin::String,
        minimum: json!(5),
        inclusive: true,
    }
}

#[test]
fn key_params_and_default_message() {
    let code = too_small();
    assert_eq!(code.key(), "too_small");
    let params = code.params();
    assert!(params.contains(&("minimum", "5".to_string())), "{params:?}");
    assert!(
        params.contains(&("inclusive", "true".to_string())),
        "{params:?}"
    );
    assert_eq!(
        code.default_message(),
        "String must be at least 5 characters"
    );
    assert_eq!(
        IssueCode::Custom {
            code: "my_code".into()
        }
        .key(),
        "my_code"
    );
    assert_eq!(IssueCode::NotInt.key(), "not_int");
}

#[test]
fn issue_and_error_constructors() {
    let issue = ValidationIssue::new(IssueCode::NotInt)
        .with_message("whole")
        .with_received(&json!(1.5));
    assert_eq!(
        (issue.message.as_str(), issue.received.clone()),
        ("whole", Some(json!(1.5)))
    );

    let e = VldError::from_code(IssueCode::NotInt);
    assert_eq!(e.issues[0].message, IssueCode::NotInt.default_message());
    assert_eq!(e.issues[0].received, None);
    let e = VldError::from_code_with_value(IssueCode::NotInt, &json!(2.5));
    assert_eq!(e.issues[0].received, Some(json!(2.5)));
    let e = VldError::single_with_value(IssueCode::NotInt, "custom", &json!("x"));
    assert_eq!(
        (e.issues[0].message.as_str(), e.issues[0].received.clone()),
        ("custom", Some(json!("x")))
    );

    let mut e = VldError::new();
    assert!(e.is_empty());
    e.push(IssueCode::NotInt, "a");
    e.push_with_value(IssueCode::NotFinite, "b", &json!(1));
    e.push_code(IssueCode::NotUnique);
    e.push_code_with_value(IssueCode::MissingContains, &json!([]));
    assert_eq!(
        codes(&e),
        ["not_int", "not_finite", "not_unique", "missing_contains"]
    );
    assert_eq!(e.issues[1].received, Some(json!(1)));
    assert_eq!(e.issues[2].message, IssueCode::NotUnique.default_message());
    assert_eq!(e.issues[3].received, Some(json!([])));

    let merged = VldError::single(IssueCode::NotInt, "x").merge(e);
    assert_eq!(merged.issues.len(), 5);
}

/// `with_prefix` prepends to every issue, so nesting builds outer-first paths.
#[test]
fn with_prefix_and_path_index() {
    let mut e = VldError::single(IssueCode::NotInt, "x");
    e.push(IssueCode::NotInt, "y");
    let e = e
        .with_prefix(PathSegment::Index(2))
        .with_prefix(PathSegment::Field("items".into()));
    assert_eq!(paths(&e), [".items[2]", ".items[2]"]);

    let mut built = VldError::new();
    built
        .issue(IssueCode::Custom { code: "c".into() })
        .path_field("rows")
        .path_index(3)
        .path_field("é")
        .received(&json!("bad"))
        .message("m")
        .finish();
    assert_eq!(path_of(&built.issues[0]), ".rows[3].é");
    assert_eq!(built.issues[0].received, Some(json!("bad")));
}

#[test]
fn issue_builder_default_message_names_the_code() {
    let mut e = VldError::new();
    e.issue(IssueCode::Custom {
        code: "my_code".into(),
    })
    .finish();
    assert!(
        e.issues[0].message.contains("my_code"),
        "{}",
        e.issues[0].message
    );
}

#[test]
fn display_lists_path_message_and_received() {
    let err = vld::object()
        .field("name", vld::string().min(2))
        .field("tags", vld::array(vld::number()))
        .parse(r#"{"name": "a", "tags": [1, "x"]}"#)
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        ".name: String must be at least 2 characters, received \"a\"\n\
         .tags[1]: Expected number, received string"
    );
}

#[test]
fn errors_round_trip_through_serde() {
    let err = vld::string().min(3).parse(r#""é""#).unwrap_err();
    let v = serde_json::to_value(&err).unwrap();
    let back: VldError = serde_json::from_value(v).unwrap();
    assert_eq!(back.issues[0].code, err.issues[0].code);
    assert_eq!(back.issues[0].message, err.issues[0].message);
}

vld::schema! {
    #[derive(Debug)]
    struct Person {
        name: String => vld::string(),
        nick: Option<String> => vld::string().optional(),
    }
}

#[test]
fn absent_required_field_is_missing_field() {
    let err = Person::parse_value(&json!({})).unwrap_err();
    assert_eq!(err.issues.len(), 1);
    assert_eq!(err.issues[0].code, IssueCode::MissingField);

    // Present but null: still a type error.
    let err = Person::parse_value(&json!({"name": null})).unwrap_err();
    assert!(matches!(err.issues[0].code, IssueCode::InvalidType { .. }));

    let obj = vld::object().field("name", vld::string());
    let err = obj.parse_value(&json!({})).unwrap_err();
    assert_eq!(err.issues[0].code, IssueCode::MissingField);
}

#[test]
fn display_does_not_repeat_received() {
    let err = vld::string().parse_value(&json!(null)).unwrap_err();
    assert_eq!(err.to_string(), "Expected string, received null");

    let err = vld::union(vld::string(), vld::number())
        .parse_value(&json!(true))
        .unwrap_err();
    let text = err.to_string();
    assert!(!text.contains("received boolean, received"), "{text}");
}
