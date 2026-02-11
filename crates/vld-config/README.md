[![Crates.io](https://img.shields.io/crates/v/vld-config?style=for-the-badge)](https://crates.io/crates/vld-config)
[![docs.rs](https://img.shields.io/docsrs/vld-config?style=for-the-badge)](https://docs.rs/vld-config)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=for-the-badge)](https://github.com/s00d/vld)
[![GitHub issues](https://img.shields.io/badge/github-issues-orange?style=for-the-badge)](https://github.com/s00d/vld/issues)
[![GitHub stars](https://img.shields.io/badge/github-stars-yellow?style=for-the-badge)](https://github.com/s00d/vld/stargazers)

# vld-config

Validate configuration files (TOML, YAML, JSON, ENV) at load time using
[vld](https://crates.io/crates/vld) schemas. Supports both
[config-rs](https://docs.rs/config) and [figment](https://docs.rs/figment).

## Installation

```toml
[dependencies]
vld = "0.1"
vld-config = "0.1"                    # config-rs backend (default)
# vld-config = { version = "0.1", features = ["figment"] }  # figment backend
# vld-config = { version = "0.1", features = ["config-rs", "figment"] }  # both
```

## Features

| Feature | Default | Description |
|---------|---------|-------------|
| `config-rs` | yes | [config](https://docs.rs/config) backend — `from_config()`, `from_builder()` |
| `figment` | no | [figment](https://docs.rs/figment) backend — `from_figment()` |

## Quick Start (config-rs)

```rust
use vld_config::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct AppSettings {
        pub host: String => vld::string().min(1),
        pub port: i64    => vld::number().int().min(1).max(65535),
        pub debug: bool  => vld::boolean(),
    }
}

let config = config::Config::builder()
    .add_source(config::File::with_name("config"))
    .add_source(config::Environment::with_prefix("APP"))
    .build()
    .unwrap();

let settings: AppSettings = from_config(&config).unwrap();
println!("{}:{}", settings.host, settings.port);
```

## Quick Start (figment)

```rust
use vld_config::from_figment;

vld::schema! {
    #[derive(Debug)]
    pub struct AppSettings {
        pub host: String => vld::string().min(1),
        pub port: i64    => vld::number().int().min(1).max(65535),
        pub debug: bool  => vld::boolean(),
    }
}

let figment = figment::Figment::new()
    .merge(figment::providers::Serialized::defaults(
        serde_json::json!({"host": "0.0.0.0", "port": 3000, "debug": false}),
    ))
    .merge(figment::providers::Env::prefixed("APP_"));

let settings: AppSettings = from_figment(&figment).unwrap();
```

## API

| Function | Backend | Description |
|----------|---------|-------------|
| `from_config(&Config)` | config-rs | Validate a built `Config` |
| `from_builder(ConfigBuilder)` | config-rs | Build and validate in one step |
| `from_figment(&Figment)` | figment | Validate a `Figment` |
| `from_value(&Value)` | — | Validate raw `serde_json::Value` |

All functions return `Result<T, VldConfigError>` where `T: VldParse` (implemented by `vld::schema!`).

## Error Handling

```rust
match from_config::<AppSettings>(&config) {
    Ok(settings) => println!("{settings:?}"),
    Err(VldConfigError::Source(msg)) => {
        eprintln!("Failed to load config: {msg}");
    }
    Err(VldConfigError::Validation(err)) => {
        eprintln!("Invalid config:");
        for issue in &err.issues {
            eprintln!("  {}: {}", issue.path_string(), issue.message);
        }
    }
}
```

## Running Examples

```bash
# config-rs
cargo run -p vld-config --example config_basic --features config-rs

# figment
cargo run -p vld-config --example figment_basic --features figment
```

## License

MIT
