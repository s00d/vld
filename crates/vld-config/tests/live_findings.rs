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
fn figment_parsed_env_fits_string_fields() {
    // Unique prefix: no other test reads these variables.
    std::env::set_var("VLDLIVE_HOST", "12345");
    std::env::set_var("VLDLIVE_PORT", "8080");
    std::env::set_var("VLDLIVE_TAGS", "true");
    let figment = figment::Figment::new().merge(figment::providers::Env::prefixed("VLDLIVE_"));
    let s: Settings = vld_config::from_figment(&figment).unwrap();
    assert_eq!(s.host, "12345");
    assert_eq!(s.port, 8080);
    assert_eq!(s.tags, vec!["true".to_string()]);
}

#[test]
fn from_value_stays_strict() {
    let v = serde_json::json!({"host": 12345, "port": 1, "tags": []});
    assert!(vld_config::from_value::<Settings>(&v).is_err());
}

/// `from_value_coerced` behaves like `from_config` / `from_figment`.
#[test]
fn from_value_coerced_matches_the_backends() {
    let v = serde_json::json!({"host": 12345, "port": 1, "tags": true});
    let s: Settings = vld_config::from_value_coerced(&v).unwrap();
    assert_eq!(s.host, "12345");
    assert_eq!(s.tags, vec!["true".to_string()]);
}
