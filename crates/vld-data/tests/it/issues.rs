//! `(path, code)` pairs of a `VldError`, for the integrations whose tests
//! compare issue lists (bson, postgres_types, clickhouse, the message
//! modules).
#![allow(dead_code)]

/// `(path, code)` of every issue of `e`.
pub fn of(e: &vld::error::VldError) -> Vec<(String, String)> {
    e.issues
        .iter()
        .map(|i| {
            (
                i.path.iter().map(ToString::to_string).collect(),
                i.code.key().to_string(),
            )
        })
        .collect()
}

/// Owned `(path, code)` pairs for comparing with [`of`].
pub fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
    expected
        .iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect()
}
