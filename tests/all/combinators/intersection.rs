use crate::common::codes;
use vld::prelude::*;

#[test]
fn both_sides_must_pass() {
    let s = vld::intersection(vld::string().min(3), vld::string().max(10));
    assert_eq!(s.parse(r#""hello""#).unwrap(), "hello");
    assert_eq!(codes(&s.parse(r#""hi""#).unwrap_err()), ["too_small"]);
    assert_eq!(
        codes(&s.parse(r#""hello world!""#).unwrap_err()),
        ["too_big"]
    );
    // `.and()` builds the same intersection.
    let and = vld::string().min(3).and(vld::string().max(10));
    assert_eq!(codes(&and.parse(r#""hi""#).unwrap_err()), ["too_small"]);
    assert!(and.parse(r#""hello""#).is_ok());
}

#[test]
fn issues_from_both_sides_are_merged() {
    let s = vld::intersection(vld::string().min(100), vld::string().email());
    let err = s.parse(r#""hi""#).unwrap_err();
    assert_eq!(codes(&err), ["too_small", "invalid_string"]);
}
