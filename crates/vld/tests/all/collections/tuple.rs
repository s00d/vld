use crate::common::{code, parse_err, paths};
use serde_json::json;
use std::f64::consts::PI;
use vld::error::{IssueCode, SizeOrigin};
use vld::prelude::*;

#[test]
fn positional_types() {
    let t = (vld::string(), vld::number().int());
    assert_eq!(
        t.parse_value(&json!(["hello", 42])).unwrap(),
        ("hello".to_string(), 42)
    );
    let t3 = (vld::string(), vld::number(), vld::boolean());
    assert_eq!(
        t3.parse_value(&json!(["hi", PI, true])).unwrap(),
        ("hi".to_string(), PI, true)
    );
}

#[test]
fn wrong_length_and_element_errors() {
    let t = (vld::string(), vld::number().int());
    for bad in [json!(["hello"]), json!(["a", 1, 2])] {
        assert_issue!(
            parse_err(&t, bad),
            "",
            IssueCode::InvalidLength {
                origin: SizeOrigin::Tuple,
                expected: 2
            }
        );
    }
    let err = parse_err(&t, json!([42, "hello"]));
    assert_eq!(paths(&err), ["[0]", "[1]"]);
    assert_issue!(
        parse_err(&t, json!({"0": "a"})),
        "",
        IssueCode::InvalidType { .. }
    );
}

#[test]
fn tuple_of_twelve() {
    let n = || vld::number().int();
    let s = (
        n(),
        n(),
        n(),
        n(),
        n(),
        n(),
        n(),
        n(),
        n(),
        n(),
        n(),
        vld::string(),
    );
    let out = s.parse(r#"[1,2,3,4,5,6,7,8,9,10,11,"z"]"#).unwrap();
    assert_eq!(out.10, 11);
    assert_eq!(out.11, "z");
    assert_eq!(
        code(s.parse("[1,2,3]")),
        IssueCode::InvalidLength {
            origin: SizeOrigin::Tuple,
            expected: 12
        }
    );
    let err = s.parse(r#"[1,2,3,4,5,6,7,8,9,10,11,12]"#).unwrap_err();
    assert_eq!(err.issues[0].path, vec![PathSegment::Index(11)]);

    let seven = (n(), n(), n(), n(), n(), n(), vld::boolean());
    assert!(seven.parse("[1,2,3,4,5,6,true]").unwrap().6);
}
