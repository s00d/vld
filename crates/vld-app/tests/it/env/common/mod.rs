//! Helpers shared by the `env` tests.
#![allow(dead_code)] // each feature combination uses a different subset

use vld_app::env::VldEnvError;

vld::schema! {
    #[derive(Debug, Clone, PartialEq)]
    pub struct Db {
        pub pool_size: i64 => vld::number().int().min(1),
        pub name: String   => vld::string(),
    }
}

vld::schema! {
    /// Flat keys, a list and a `db` section (`APP_DB__POOL_SIZE`).
    #[derive(Debug, Clone, PartialEq)]
    pub struct Settings {
        pub host: String      => vld::string().min(1),
        pub port: i64         => vld::number().int().min(1).max(65535),
        pub debug: bool       => vld::boolean(),
        pub tags: Vec<String> => vld::array(vld::string()),
        pub db: Db            => vld::nested(Db::parse_value),
    }
}

/// `(variable, code)` of every issue, or a panic for another error or success.
#[track_caller]
pub fn var_issues<T: std::fmt::Debug>(r: Result<T, VldEnvError>) -> Vec<(String, String)> {
    match r {
        Err(e @ VldEnvError::Validation { .. }) => e
            .var_issues()
            .map(|(var, i)| (var.to_string(), i.code.key().to_string()))
            .collect(),
        other => panic!("expected a validation error, got {other:?}"),
    }
}

/// Owned `(variable, code)` pairs to compare with [`var_issues`].
pub fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
    expected
        .iter()
        .map(|(v, c)| (v.to_string(), c.to_string()))
        .collect()
}
