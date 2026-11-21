//! Stories: an app loads its settings with config-rs.

use super::common::*;

use std::error::Error;
use vld_app::config::{VldConfigError, from_builder, from_config};

/// Env vars as config-rs reads them (`APP_` prefix, `__` between sections).
fn env(vars: &[(&str, &str)]) -> config::Environment {
    let map = vars
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    config::Environment::with_prefix("APP")
        .prefix_separator("_")
        .separator("__")
        .source(Some(map))
}

/// Given `app.toml` with flat keys and a `[db]` section, and env vars
/// overriding a flat key, a nested key and a list (`try_parsing(true)`),
/// when the app loads it with `from_builder` and with `from_config`,
/// then it gets typed settings: file values, env values on top, a numeric
/// env value turned back into text for a string field and a single env value
/// wrapped into the list.
#[test]
fn toml_file_with_env_layer() {
    let (_dir, path) = file("app.toml", APP_TOML);

    let s: Settings =
        from_builder(config::Config::builder().add_source(config::File::from(path.clone())))
            .unwrap();
    assert_eq!(
        s,
        Settings {
            host: "127.0.0.1".into(),
            port: 3000,
            debug: false,
            db: Db {
                url: "https://db.example.com".into(),
                pool_size: 10,
                name: None,
            },
            tags: vec!["web".into()],
        }
    );

    let vars = [
        ("APP_PORT", "9090"),
        ("APP_HOST", "12345"),
        ("APP_DB__POOL_SIZE", "20"),
        ("APP_DB__NAME", "main"),
        ("APP_TAGS", "7"),
    ];
    let config = config::Config::builder()
        .add_source(config::File::from(path))
        .add_source(env(&vars).try_parsing(true))
        .build()
        .unwrap();
    let s: Settings = from_config(&config).unwrap();
    assert_eq!((s.host.as_str(), s.port, s.debug), ("12345", 9090, false));
    assert_eq!((s.db.pool_size, s.db.name.as_deref()), (20, Some("main")));
    assert_eq!(s.db.url, "https://db.example.com");
    assert_eq!(s.tags, ["7"]);
}

/// Given env vars only, with and without `try_parsing`,
/// when the app loads flat settings from them,
/// then without parsing every value is text and the number / boolean fields
/// fail with `invalid_type`; with parsing they fit, string fields get the
/// backend's parsed text back (`01234` → `"1234"`, documented as lossy).
#[test]
fn env_values_need_try_parsing_for_numbers_and_booleans() {
    let vars = [
        ("APP_HOST", "h"),
        ("APP_PORT", "8080"),
        ("APP_DEBUG", "true"),
        ("APP_TAGS", "a"),
    ];
    assert_eq!(
        issues(from_builder::<Flat>(
            config::Config::builder().add_source(env(&vars))
        )),
        pairs(&[(".port", "invalid_type"), (".debug", "invalid_type")])
    );

    let s: Flat =
        from_builder(config::Config::builder().add_source(env(&vars).try_parsing(true))).unwrap();
    assert_eq!((s.host.as_str(), s.port, s.debug), ("h", 8080, true));
    assert_eq!(s.tags, ["a"]);

    let lossy = [
        ("APP_HOST", "01234"),
        ("APP_PORT", "1"),
        ("APP_DEBUG", "FALSE"),
        ("APP_TAGS", "true"),
    ];
    let s: Flat =
        from_builder(config::Config::builder().add_source(env(&lossy).try_parsing(true))).unwrap();
    assert_eq!((s.host.as_str(), s.debug), ("1234", false));
    assert_eq!(s.tags, ["true"]);
}

/// Given a broken `bad.toml` (empty host, port out of range, a bad URL and a
/// missing key in `[db]`),
/// when the app loads it,
/// then it gets one validation error listing every bad key by its path, a
/// `Display` prefixed `validation error: ` and the `VldError` as `source()`.
#[test]
fn broken_file_lists_every_bad_key() {
    let (_dir, path) = file(
        "bad.toml",
        "host = \"\"\nport = 99999\ndebug = true\ntags = []\n[db]\nurl = \"not-a-url\"\n",
    );
    let config = config::Config::builder()
        .add_source(config::File::from(path))
        .build()
        .unwrap();

    let err = from_config::<Settings>(&config).unwrap_err();
    assert!(err.to_string().starts_with("validation error: "), "{err}");
    let source = err.source().expect("the VldError");
    assert!(source.downcast_ref::<vld::error::VldError>().is_some());

    assert_eq!(
        issues(from_config::<Settings>(&config)),
        pairs(&[
            (".host", "too_small"),
            (".port", "too_big"),
            (".db.url", "invalid_string"),
            (".db.pool_size", "missing_field"),
        ])
    );
    let VldConfigError::Validation(e) = err else {
        unreachable!()
    };
    assert_eq!(
        e.issues[2].code.params(),
        [("validation", "url".to_string())]
    );
}

/// Given a JSON config file and a YAML one,
/// when the app loads them,
/// then the format makes no difference: same typed settings, an optional key
/// present or absent.
#[test]
fn json_and_yaml_files() {
    let (_d1, json) = file(
        "app.json",
        r#"{"host": "0.0.0.0", "port": 443, "debug": false, "tags": [],
            "db": {"url": "https://db.example.com", "pool_size": 5, "name": "mydb"}}"#,
    );
    let s: Settings =
        from_builder(config::Config::builder().add_source(config::File::from(json))).unwrap();
    assert_eq!((s.port, s.db.name.as_deref()), (443, Some("mydb")));

    let (_d2, yaml) = file(
        "app.yaml",
        "host: example.org\nport: 80\ndebug: true\ntags: [a, b]\ndb:\n  url: https://x.io\n  pool_size: 1\n",
    );
    let s: Settings =
        from_builder(config::Config::builder().add_source(config::File::from(yaml))).unwrap();
    assert_eq!(
        (s.host.as_str(), s.port, s.debug),
        ("example.org", 80, true)
    );
    assert_eq!((s.db.pool_size, s.db.name), (1, None));
    assert_eq!(s.tags, ["a", "b"]);
}

/// Given a required config file that does not exist, and one with a TOML
/// syntax error,
/// when the app loads them,
/// then it gets a source error (`config error: …`, no `source()`), not a
/// validation error.
#[test]
fn unreadable_files_are_source_errors() {
    let dir = tempfile::tempdir().unwrap();
    let missing = config::File::from(dir.path().join("nope.toml")).required(true);
    let err = from_builder::<Settings>(config::Config::builder().add_source(missing)).unwrap_err();
    assert!(err.to_string().starts_with("config error: "), "{err}");
    assert!(err.source().is_none());
    assert!(matches!(err, VldConfigError::Source(_)));

    let (_d, broken) = file("broken.toml", "host = \n");
    let msg = source_msg(from_builder::<Settings>(
        config::Config::builder().add_source(config::File::from(broken)),
    ));
    assert!(!msg.is_empty());
}
