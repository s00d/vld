use crate::common::{parse_err, paths};
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
