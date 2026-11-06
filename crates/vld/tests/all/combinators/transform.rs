//! `transform` and `preprocess`.

use crate::common::{codes, parse_err};
use serde_json::json;
use vld::prelude::*;

#[test]
fn transform_maps_the_output() {
    let len = vld::string().transform(|s| s.chars().count());
    assert_eq!(len.parse_value(&json!("héllo")).unwrap(), 5);
    assert_eq!(codes(&parse_err(&len, json!(1))), ["invalid_type"]);

    let defaulted = vld::string()
        .optional()
        .transform(|opt| opt.unwrap_or_else(|| "default".to_string()));
    assert_eq!(defaulted.parse_value(&json!(null)).unwrap(), "default");
    assert_eq!(defaulted.parse_value(&json!("hello")).unwrap(), "hello");
}

#[test]
fn preprocess_rewrites_the_input() {
    let s = vld::preprocess(
        |v| match v.as_str() {
            Some(s) => json!(s.trim()),
            None => v.clone(),
        },
        vld::string().min(1),
    );
    assert_eq!(s.parse_value(&json!("  hello  ")).unwrap(), "hello");
    assert_eq!(codes(&parse_err(&s, json!("   "))), ["too_small"]);
    assert_eq!(codes(&parse_err(&s, json!(5))), ["invalid_type"]);
}
