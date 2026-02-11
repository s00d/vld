use std::io::Write;
use vld::prelude::VldSchema;
// -- Shared schema --

vld::schema! {
    #[derive(Debug)]
    pub struct AppSettings {
        pub host: String => vld::string().min(1),
        pub port: i64    => vld::number().int().min(1).max(65535),
        pub debug: bool  => vld::boolean(),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct DbSettings {
        pub url: String          => vld::string().min(1).url(),
        pub pool_size: i64       => vld::number().int().min(1).max(100),
        pub name: Option<String> => vld::string().min(1).optional(),
    }
}

// =========================================================================
// config-rs backend
// =========================================================================

#[cfg(feature = "config-rs")]
mod config_rs_tests {
    use super::*;
    use vld_config::{from_builder, from_config, from_value, VldConfigError};

    #[test]
    fn from_value_valid() {
        let json = serde_json::json!({"host": "0.0.0.0", "port": 8080, "debug": true});
        let settings: AppSettings = from_value(&json).unwrap();
        assert_eq!(settings.host, "0.0.0.0");
        assert_eq!(settings.port, 8080);
        assert!(settings.debug);
    }

    #[test]
    fn from_value_invalid() {
        let json = serde_json::json!({"host": "", "port": 0, "debug": true});
        let err = from_value::<AppSettings>(&json).unwrap_err();
        match err {
            VldConfigError::Validation(e) => {
                assert!(e.issues.len() >= 2); // host too short, port < 1
            }
            _ => panic!("Expected validation error"),
        }
    }

    #[test]
    fn from_value_missing_fields() {
        let json = serde_json::json!({"host": "localhost"});
        let err = from_value::<AppSettings>(&json).unwrap_err();
        match err {
            VldConfigError::Validation(e) => {
                assert!(!e.issues.is_empty());
            }
            _ => panic!("Expected validation error"),
        }
    }

    #[test]
    fn from_config_toml_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, r#"host = "127.0.0.1""#).unwrap();
        writeln!(f, "port = 3000").unwrap();
        writeln!(f, "debug = false").unwrap();

        let config = config::Config::builder()
            .add_source(config::File::from(path))
            .build()
            .unwrap();

        let settings: AppSettings = from_config(&config).unwrap();
        assert_eq!(settings.host, "127.0.0.1");
        assert_eq!(settings.port, 3000);
        assert!(!settings.debug);
    }

    #[test]
    fn from_config_toml_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, r#"host = """#).unwrap();
        writeln!(f, "port = 99999").unwrap();
        writeln!(f, "debug = true").unwrap();

        let config = config::Config::builder()
            .add_source(config::File::from(path))
            .build()
            .unwrap();

        let err = from_config::<AppSettings>(&config).unwrap_err();
        match err {
            VldConfigError::Validation(e) => {
                // host: min(1) fails, port: max(65535) fails
                assert!(e.issues.len() >= 2);
            }
            _ => panic!("Expected validation error"),
        }
    }

    #[test]
    fn from_builder_valid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("builder.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, r#"host = "localhost""#).unwrap();
        writeln!(f, "port = 8080").unwrap();
        writeln!(f, "debug = true").unwrap();

        let settings: AppSettings =
            from_builder(config::Config::builder().add_source(config::File::from(path))).unwrap();
        assert_eq!(settings.host, "localhost");
    }

    #[test]
    fn from_config_json_source() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.json");
        std::fs::write(&path, r#"{"host": "0.0.0.0", "port": 443, "debug": false}"#).unwrap();

        let config = config::Config::builder()
            .add_source(config::File::from(path))
            .build()
            .unwrap();

        let settings: AppSettings = from_config(&config).unwrap();
        assert_eq!(settings.port, 443);
    }

    #[test]
    fn from_config_with_optional_field() {
        let json = serde_json::json!({
            "url": "https://db.example.com",
            "pool_size": 10
        });
        let settings: DbSettings = from_value(&json).unwrap();
        assert_eq!(settings.pool_size, 10);
        assert!(settings.name.is_none());
    }

    #[test]
    fn from_config_with_optional_field_present() {
        let json = serde_json::json!({
            "url": "https://db.example.com",
            "pool_size": 5,
            "name": "mydb"
        });
        let settings: DbSettings = from_value(&json).unwrap();
        assert_eq!(settings.name.as_deref(), Some("mydb"));
    }

    #[test]
    fn from_config_url_invalid() {
        let json = serde_json::json!({
            "url": "not-a-url",
            "pool_size": 5
        });
        let err = from_value::<DbSettings>(&json).unwrap_err();
        match err {
            VldConfigError::Validation(e) => {
                assert!(e.issues.iter().any(|i| i.message.contains("URL")));
            }
            _ => panic!("Expected validation error"),
        }
    }

    #[test]
    fn error_display() {
        let json = serde_json::json!({"host": "", "port": 0, "debug": true});
        let err = from_value::<AppSettings>(&json).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("validation error"));
    }

    #[test]
    fn source_error_display() {
        let err = VldConfigError::Source("test error".into());
        assert_eq!(format!("{err}"), "config error: test error");
    }
}

// =========================================================================
// figment backend
// =========================================================================

#[cfg(feature = "figment")]
mod figment_tests {
    use super::*;
    use figment::providers::Format;
    use vld_config::{from_figment, VldConfigError};

    #[test]
    fn from_figment_defaults_valid() {
        let figment = figment::Figment::new().merge(figment::providers::Serialized::defaults(
            serde_json::json!({"host": "0.0.0.0", "port": 8080, "debug": false}),
        ));

        let settings: AppSettings = from_figment(&figment).unwrap();
        assert_eq!(settings.host, "0.0.0.0");
        assert_eq!(settings.port, 8080);
    }

    #[test]
    fn from_figment_invalid() {
        let figment = figment::Figment::new().merge(figment::providers::Serialized::defaults(
            serde_json::json!({"host": "", "port": 0, "debug": true}),
        ));

        let err = from_figment::<AppSettings>(&figment).unwrap_err();
        match err {
            VldConfigError::Validation(e) => {
                assert!(e.issues.len() >= 2);
            }
            _ => panic!("Expected validation error"),
        }
    }

    #[test]
    fn from_figment_toml_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("figment.toml");
        std::fs::write(&path, "host = \"localhost\"\nport = 9090\ndebug = true\n").unwrap();

        let figment = figment::Figment::new().merge(figment::providers::Toml::file(&path));

        let settings: AppSettings = from_figment(&figment).unwrap();
        assert_eq!(settings.host, "localhost");
        assert_eq!(settings.port, 9090);
        assert!(settings.debug);
    }

    #[test]
    fn from_figment_merge_layers() {
        let figment = figment::Figment::new()
            .merge(figment::providers::Serialized::defaults(
                serde_json::json!({"host": "0.0.0.0", "port": 3000, "debug": false}),
            ))
            .merge(figment::providers::Serialized::defaults(
                serde_json::json!({"port": 9090}),
            ));

        let settings: AppSettings = from_figment(&figment).unwrap();
        assert_eq!(settings.host, "0.0.0.0"); // from first layer
        assert_eq!(settings.port, 9090); // overridden by second
    }

    #[test]
    fn from_figment_extraction_error() {
        // Empty figment — extraction should fail
        let figment = figment::Figment::new();
        let err = from_figment::<AppSettings>(&figment).unwrap_err();
        match err {
            VldConfigError::Source(_) | VldConfigError::Validation(_) => {} // both are fine
        }
    }
}
