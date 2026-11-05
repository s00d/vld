//! `boolean`, `enumeration`, `literal`, `any`.

use crate::common::parse_err;
use serde_json::json;
use vld::error::IssueCode;
use vld::prelude::*;

#[test]
fn boolean_strict_and_coerce() {
    let b = vld::boolean();
    assert!(b.parse_value(&json!(true)).unwrap());
    assert!(!b.parse_value(&json!(false)).unwrap());
    for bad in [json!("true"), json!(1), json!(null)] {
        assert_issue!(parse_err(&b, bad), "", IssueCode::InvalidType { expected, .. } if expected == "boolean");
    }

    let c = vld::boolean().coerce();
    let table = [
        (json!("true"), true),
        (json!("1"), true),
        (json!("false"), false),
        (json!("0"), false),
        (json!(0), false),
        (json!(0.0), false),
        (json!(2), true),
        (json!(-1), true),
    ];
    for (input, want) in table {
        assert_eq!(c.parse_value(&input).unwrap(), want, "{input}");
    }
    for bad in ["TRUE", "yes", "", "١"] {
        let err = parse_err(&c, json!(bad));
        assert_issue!(&err, "", IssueCode::InvalidType { .. });
        assert_eq!(
            err.issues[0].message,
            "Expected boolean, received string"
        );
    }
    assert_issue!(
        parse_err(&c, json!(null)),
        "",
        IssueCode::InvalidType { .. }
    );
    let e = parse_err(&vld::boolean().type_error("bool!"), json!(1));
    assert_eq!(e.issues[0].message, "bool!");
}

#[test]
fn literals_compare_by_value() {
    let l = vld::literal("admin");
    assert_eq!(l.parse_value(&json!("admin")).unwrap(), "admin");
    assert_eq!(l.expected_value(), &json!("admin"));
    assert_issue!(
        parse_err(&l, json!("Admin")),
        "",
        IssueCode::InvalidLiteral { expected } if expected == &json!("admin")
    );

    let l = vld::literal(42i64);
    assert_eq!(l.parse_value(&json!(42)).unwrap(), 42);
    assert_issue!(
        parse_err(&l, json!(43)),
        "",
        IssueCode::InvalidLiteral { .. }
    );
    assert_issue!(
        parse_err(&l, json!("42")),
        "",
        IssueCode::InvalidLiteral { .. }
    );

    assert!(vld::literal(true).parse_value(&json!(true)).unwrap());
    assert_issue!(
        parse_err(&vld::literal(true), json!(false)),
        "",
        IssueCode::InvalidLiteral { .. }
    );

    // Numbers compare numerically, not by JSON representation.
    assert_eq!(vld::literal(1.0).parse("1").unwrap(), 1.0);
    assert_eq!(vld::literal(5i64).parse("5.0").unwrap(), 5);
    assert!(vld::literal(5i64).parse("5.5").is_err());
}

#[test]
fn enumeration() {
    let e = vld::enumeration(&["red", "green", "blue"]);
    assert_eq!(e.parse_value(&json!("red")).unwrap(), "red");
    let err = parse_err(&e, json!("yellow"));
    assert_issue!(
        &err,
        "",
        IssueCode::InvalidEnumValue { options } if options == &["red", "green", "blue"]
    );
    assert_issue!(
        parse_err(&e, json!("RED")),
        "",
        IssueCode::InvalidEnumValue { .. }
    );
    assert_issue!(parse_err(&e, json!(42)), "", IssueCode::InvalidType { .. });

    let e = ZEnum::from_strings(vec!["é".to_string(), "b".to_string()]);
    assert_eq!(e.parse_value(&json!("é")).unwrap(), "é");
    assert_issue!(parse_err(&e, json!("e")), "", IssueCode::InvalidEnumValue { options } if options.len() == 2);

    let e = vld::enumeration(&["a"]).type_error("enum!");
    assert_eq!(parse_err(&e, json!(1)).issues[0].message, "enum!");
}

#[test]
fn any_accepts_everything() {
    let a = vld::any();
    for v in [
        json!("hello"),
        json!(42),
        json!(null),
        json!([1, {"a": []}]),
    ] {
        assert_eq!(a.parse_value(&v).unwrap(), v);
    }
}
