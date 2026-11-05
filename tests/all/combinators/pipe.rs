use crate::common::{codes, parse_err};
use serde_json::json;
use vld::prelude::*;

#[test]
fn pipe_feeds_the_output_into_the_next_schema() {
    let s = vld::string()
        .transform(|s| s.len())
        .pipe(vld::number().min(3.0));
    assert_eq!(s.parse_value(&json!("hello")).unwrap(), 5.0);
    assert_eq!(codes(&parse_err(&s, json!("hi"))), ["too_small"]);
    // The first stage's error stops the pipe.
    assert_eq!(codes(&parse_err(&s, json!(1))), ["invalid_type"]);
}

#[test]
fn accessors_expose_both_stages() {
    let s = vld::string().pipe(vld::string().min(3));
    assert_eq!(s.first_schema().parse_value(&json!("a")).unwrap(), "a");
    assert_eq!(
        codes(&parse_err(s.second_schema(), json!("a"))),
        ["too_small"]
    );
}
