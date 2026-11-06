//! Helpers shared by the vld-sea test binaries (sea-orm 1 / sea-orm 2).
#![allow(dead_code)] // each test binary uses a different subset

use vld_sea::VldSeaError;

/// `(path, code)` of every issue of a `Validation` error (panics on `Ok`).
#[track_caller]
pub fn issues<T: std::fmt::Debug>(r: Result<T, VldSeaError>) -> Vec<(String, String)> {
    match r {
        Err(VldSeaError::Validation(e)) => e
            .issues
            .iter()
            .map(|i| {
                (
                    i.path.iter().map(ToString::to_string).collect(),
                    i.code.key().to_string(),
                )
            })
            .collect(),
        other => panic!("expected a validation error, got {other:?}"),
    }
}

/// Owned `(path, code)` pairs for comparing with [`issues`].
pub fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
    expected
        .iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect()
}
