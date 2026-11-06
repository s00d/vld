//! Helpers shared by the vld-schemars test binaries.
#![allow(dead_code)] // each test binary uses a different subset

use serde_json::Value;
use vld::error::VldError;

/// `(path, code key)` of every issue, in order (`""` is the root path).
pub fn pairs(err: &VldError) -> Vec<(String, String)> {
    err.issues
        .iter()
        .map(|i| {
            (
                i.path.iter().map(ToString::to_string).collect(),
                i.code.key().to_string(),
            )
        })
        .collect()
}

/// `value` passes `schema` (`validate_with_schema`).
#[track_caller]
pub fn valid(schema: &Value, value: &Value) {
    if let Err(e) = vld_schemars::validate_with_schema(schema, value) {
        panic!("{value} rejected by {schema}: {:?}", pairs(&e));
    }
}

/// `value` fails `schema` with exactly these `(path, code)` issues.
#[track_caller]
pub fn invalid(schema: &Value, value: &Value, expected: &[(&str, &str)]) {
    match vld_schemars::validate_with_schema(schema, value) {
        Ok(()) => panic!("{value} accepted by {schema}"),
        Err(e) => {
            let got = pairs(&e);
            let got: Vec<(&str, &str)> =
                got.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
            assert_eq!(got, expected, "{value} against {schema}");
        }
    }
}
