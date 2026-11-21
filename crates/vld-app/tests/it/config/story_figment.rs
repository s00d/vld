//! Stories: an app loads its settings with figment.

#![allow(clippy::result_large_err)] // figment::Jail closures return figment::Error

use super::common::*;

use figment::providers::{Env, Format, Serialized, Toml};
use figment::{Figment, Jail};
use serde_json::json;
use vld_app::config::{VldConfigError, from_figment};

/// Given defaults in code, an `app.toml` file and `APP_` env vars (`__`
/// between sections), merged in that order,
/// when the app extracts its settings,
/// then later layers win key by key; env text fits string fields and a
/// single value fits the list.
#[test]
fn defaults_file_and_env_layers() {
    Jail::expect_with(|jail| {
        jail.create_file(
            "app.toml",
            "port = 9090\ntags = [\"web\"]\n[db]\nurl = \"https://db.example.com\"\npool_size = 10\n",
        )?;
        let base = Figment::new()
            .merge(Serialized::defaults(json!({
                "host": "0.0.0.0", "port": 3000, "debug": false, "tags": []
            })))
            .merge(Toml::file("app.toml"));
        let s: Settings = from_figment(&base).unwrap();
        assert_eq!((s.host.as_str(), s.port, s.debug), ("0.0.0.0", 9090, false));
        assert_eq!(
            (s.db.pool_size, s.tags.as_slice()),
            (10, ["web".to_string()].as_slice())
        );

        jail.set_env("APP_HOST", "12345");
        jail.set_env("APP_DEBUG", "true");
        jail.set_env("APP_DB__POOL_SIZE", "20");
        jail.set_env("APP_TAGS", "true");
        let layered = base.merge(Env::prefixed("APP_").split("__"));
        let s: Settings = from_figment(&layered).unwrap();
        assert_eq!((s.host.as_str(), s.port, s.debug), ("12345", 9090, true));
        assert_eq!(s.db.pool_size, 20);
        assert_eq!(s.tags, ["true"]);
        Ok(())
    });
}

/// Given an empty figment and one with invalid defaults,
/// when the app extracts its settings,
/// then the empty one reports every required key `missing_field`, the invalid
/// one every bad key.
#[test]
fn missing_and_invalid_keys() {
    assert_eq!(
        issues(from_figment::<Flat>(&Figment::new())),
        pairs(&[
            (".host", "missing_field"),
            (".port", "missing_field"),
            (".debug", "missing_field"),
            (".tags", "missing_field"),
        ])
    );

    let bad = Figment::new().merge(Serialized::defaults(json!({
        "host": "", "port": 0, "debug": true, "tags": [],
        "db": {"url": "https://x.io", "pool_size": 101}
    })));
    assert_eq!(
        issues(from_figment::<Settings>(&bad)),
        pairs(&[
            (".host", "too_small"),
            (".port", "too_small"),
            (".db.pool_size", "too_big")
        ])
    );
}

/// Given a TOML file with a syntax error,
/// when the app extracts its settings,
/// then it gets a source error, not a validation error.
#[test]
fn broken_file_is_a_source_error() {
    Jail::expect_with(|jail| {
        jail.create_file("app.toml", "host = \n")?;
        let err = from_figment::<Flat>(&Figment::new().merge(Toml::file("app.toml"))).unwrap_err();
        assert!(matches!(err, VldConfigError::Source(_)), "{err:?}");
        assert!(err.to_string().starts_with("config error: "), "{err}");
        Ok(())
    });
}
