//! Helpers shared by the modules of the `it` test binary.
#![allow(dead_code)] // each feature combination uses a different subset

use vld_config::VldConfigError;

vld::schema! {
    #[derive(Debug, Clone, PartialEq)]
    pub struct Db {
        pub url: String          => vld::string().min(1).url(),
        pub pool_size: i64       => vld::number().int().min(1).max(100),
        pub name: Option<String> => vld::string().min(1).optional(),
    }
}

vld::schema! {
    /// The app's settings: flat keys, a nested `[db]` section, a list.
    #[derive(Debug, Clone, PartialEq)]
    pub struct Settings {
        pub host: String       => vld::string().min(1),
        pub port: i64          => vld::number().int().min(1).max(65535),
        pub debug: bool        => vld::boolean(),
        pub db: Db             => vld::nested(Db::parse_value),
        pub tags: Vec<String>  => vld::array(vld::string()),
    }
}

vld::schema! {
    /// Flat settings, as env vars give them.
    #[derive(Debug, Clone, PartialEq)]
    pub struct Flat {
        pub host: String       => vld::string().min(1),
        pub port: i64          => vld::number().int().min(1).max(65535),
        pub debug: bool        => vld::boolean(),
        pub tags: Vec<String>  => vld::array(vld::string()),
    }
}

/// `(path, code)` of every issue (`.db.url`), or a panic for a source error
/// or success.
#[track_caller]
pub fn issues<T: std::fmt::Debug>(r: Result<T, VldConfigError>) -> Vec<(String, String)> {
    match r {
        Err(VldConfigError::Validation(e)) => e
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

/// Owned `(path, code)` pairs to compare with [`issues`].
pub fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
    expected
        .iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect()
}

/// The source message, or a panic for a validation error or success.
#[track_caller]
pub fn source_msg<T: std::fmt::Debug>(r: Result<T, VldConfigError>) -> String {
    match r {
        Err(VldConfigError::Source(m)) => m,
        other => panic!("expected a source error, got {other:?}"),
    }
}

/// A temp dir holding `name` with `contents`; returns the dir (keep it alive)
/// and the file path.
pub fn file(name: &str, contents: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(name);
    std::fs::write(&path, contents).unwrap();
    (dir, path)
}

pub const APP_TOML: &str = r#"
host = "127.0.0.1"
port = 3000
debug = false
tags = ["web"]

[db]
url = "https://db.example.com"
pool_size = 10
"#;
