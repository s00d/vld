//! Regression tests for the live end-to-end findings (0.5.0).

vld::schema! {
    #[derive(Debug)]
    pub struct Settings {
        pub host: String => vld::string().min(1),
        pub port: i64 => vld::number().int(),
        pub tags: Vec<String> => vld::array(vld::string()),
    }
}

#[cfg(feature = "config-rs")]
#[test]
fn config_rs_parsed_env_fits_string_fields() {
    let vars = [
        ("APP_HOST", "12345"),
        ("APP_PORT", "8080"),
        ("APP_TAGS", "7"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    let env = config::Environment::with_prefix("APP")
        .source(Some(vars))
        .try_parsing(true);
    let s: Settings = vld_config::from_builder(config::Config::builder().add_source(env)).unwrap();
    assert_eq!(s.host, "12345");
    assert_eq!(s.port, 8080);
    assert_eq!(s.tags, vec!["7".to_string()]);
}

#[cfg(feature = "figment")]
#[test]
#[allow(clippy::result_large_err)] // figment::Jail closures return figment::Error
fn figment_parsed_env_fits_string_fields() {
    // figment's Jail serializes env changes and restores them afterwards.
    figment::Jail::expect_with(|jail| {
        jail.set_env("VLDLIVE_HOST", "12345");
        jail.set_env("VLDLIVE_PORT", "8080");
        jail.set_env("VLDLIVE_TAGS", "true");
        let figment = figment::Figment::new().merge(figment::providers::Env::prefixed("VLDLIVE_"));
        let s: Settings = vld_config::from_figment(&figment).unwrap();
        assert_eq!(s.host, "12345");
        assert_eq!(s.port, 8080);
        assert_eq!(s.tags, vec!["true".to_string()]);
        Ok(())
    });
}

#[test]
fn from_value_stays_strict() {
    let v = serde_json::json!({"host": 12345, "port": 1, "tags": []});
    match vld_config::from_value::<Settings>(&v) {
        Err(vld_config::VldConfigError::Validation(e)) => {
            assert_eq!(e.issues.len(), 1, "{e}");
            assert_eq!(e.issues[0].code.key(), "invalid_type");
            assert_eq!(
                e.issues[0]
                    .path
                    .iter()
                    .map(ToString::to_string)
                    .collect::<String>(),
                ".host"
            );
        }
        other => panic!("expected a validation error, got {other:?}"),
    }
}

/// `from_value_coerced` behaves like `from_config` / `from_figment`.
#[test]
fn from_value_coerced_matches_the_backends() {
    let v = serde_json::json!({"host": 12345, "port": 1, "tags": true});
    let s: Settings = vld_config::from_value_coerced(&v).unwrap();
    assert_eq!(s.host, "12345");
    assert_eq!(s.tags, vec!["true".to_string()]);
}
