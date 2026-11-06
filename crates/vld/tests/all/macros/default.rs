//! `impl_default!`: field-wise `Default` (type defaults, not schema defaults).

vld::schema! {
    #[derive(Debug, PartialEq)]
    pub struct Config {
        pub host: String => vld::string().with_default("localhost".into()),
        pub port: Option<i64> => vld::number().int().optional(),
        pub tags: Vec<String> => vld::array(vld::string()).with_default(vec![]),
    }
}

vld::impl_default!(Config { host, port, tags });

#[test]
fn default_uses_type_defaults_but_parse_uses_schema_defaults() {
    assert_eq!(
        Config::default(),
        Config {
            host: String::new(),
            port: None,
            tags: vec![],
        }
    );
    let parsed = Config::parse("{}").unwrap();
    assert_eq!(parsed.host, "localhost");
    let parsed = Config::parse(r#"{"host": "example.com", "port": 8080}"#).unwrap();
    assert_eq!(
        (parsed.host.as_str(), parsed.port),
        ("example.com", Some(8080))
    );
}
