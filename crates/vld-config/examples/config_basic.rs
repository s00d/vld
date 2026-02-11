use vld_config::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct AppSettings {
        pub host: String => vld::string().min(1),
        pub port: i64    => vld::number().int().min(1).max(65535),
        pub debug: bool  => vld::boolean(),
    }
}

fn main() {
    // Example 1: From a JSON value (simulating config source)
    println!("=== Valid config ===");
    let json = serde_json::json!({
        "host": "0.0.0.0",
        "port": 8080,
        "debug": true
    });
    match from_value::<AppSettings>(&json) {
        Ok(settings) => println!("{settings:?}"),
        Err(e) => println!("Error: {e}"),
    }

    // Example 2: Invalid config
    println!("\n=== Invalid config ===");
    let bad_json = serde_json::json!({
        "host": "",
        "port": 99999,
        "debug": true
    });
    match from_value::<AppSettings>(&bad_json) {
        Ok(settings) => println!("{settings:?}"),
        Err(e) => println!("Error: {e}"),
    }

    // Example 3: From a TOML file via config-rs
    println!("\n=== From TOML file ===");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.toml");
    std::fs::write(&path, "host = \"localhost\"\nport = 3000\ndebug = false\n").unwrap();

    let config = config::Config::builder()
        .add_source(config::File::from(path))
        .build()
        .unwrap();

    match from_config::<AppSettings>(&config) {
        Ok(settings) => println!("{settings:?}"),
        Err(e) => println!("Error: {e}"),
    }
}
