use vld_config::from_figment;

vld::schema! {
    #[derive(Debug)]
    pub struct AppSettings {
        pub host: String => vld::string().min(1),
        pub port: i64    => vld::number().int().min(1).max(65535),
        pub debug: bool  => vld::boolean(),
    }
}

fn main() {
    // Figment with defaults + overrides
    println!("=== Figment: defaults + override ===");
    let figment = figment::Figment::new()
        .merge(figment::providers::Serialized::defaults(
            serde_json::json!({
                "host": "0.0.0.0",
                "port": 3000,
                "debug": false
            }),
        ))
        // Override port from "env" (simulated with Serialized)
        .merge(figment::providers::Serialized::defaults(
            serde_json::json!({"port": 9090, "debug": true}),
        ));

    match from_figment::<AppSettings>(&figment) {
        Ok(settings) => println!("{settings:?}"),
        Err(e) => println!("Error: {e}"),
    }

    // Invalid config
    println!("\n=== Figment: invalid ===");
    let figment = figment::Figment::new().merge(figment::providers::Serialized::defaults(
        serde_json::json!({"host": "", "port": 0, "debug": true}),
    ));

    match from_figment::<AppSettings>(&figment) {
        Ok(settings) => println!("{settings:?}"),
        Err(e) => println!("Error: {e}"),
    }
}
