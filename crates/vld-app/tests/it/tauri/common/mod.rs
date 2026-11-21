//! Helpers shared by the modules of the `it` test binary.
#![allow(dead_code)]

use std::collections::BTreeMap;

use vld::error::VldError;
use vld_app::tauri::{TauriIssue, VldTauriError};

/// A command payload type that fails with exactly the error carried in the
/// payload (`{"fail": <VldError>}`) and accepts anything else: lets a test
/// put any issue list through the crate's real entry points.
#[derive(Debug)]
pub struct Echo;

impl vld::schema::VldParse for Echo {
    fn vld_parse_value(value: &serde_json::Value) -> Result<Self, VldError> {
        match value.get("fail") {
            Some(e) => Err(serde_json::from_value(e.clone()).expect("a serialized VldError")),
            None => Ok(Echo),
        }
    }
}

/// The payload that makes [`Echo`] fail with `err`.
pub fn failing(err: &VldError) -> serde_json::Value {
    serde_json::json!({ "fail": err })
}

/// `(path, code)` of every frontend issue, in order.
pub fn paths_codes(e: &VldTauriError) -> Vec<(&str, &str)> {
    e.issues
        .iter()
        .map(|i| (i.path.as_str(), i.code.as_str()))
        .collect()
}

/// Owned `params` map from pairs.
pub fn params(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// The issue at `path` (panics when absent).
#[track_caller]
pub fn issue<'a>(e: &'a VldTauriError, path: &str) -> &'a TauriIssue {
    e.issues
        .iter()
        .find(|i| i.path == path)
        .unwrap_or_else(|| panic!("no issue at {path:?}: {e:?}"))
}
