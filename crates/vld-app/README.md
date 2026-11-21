[![Crates.io](https://img.shields.io/crates/v/vld-app?style=for-the-badge)](https://crates.io/crates/vld-app)
[![Downloads](https://img.shields.io/crates/d/vld-app?style=for-the-badge)](https://crates.io/crates/vld-app)
[![docs.rs](https://img.shields.io/badge/docs-docs.rs-blue?style=for-the-badge)](https://docs.rs/vld-app)
[![Guide](https://img.shields.io/badge/guide-s00d.github.io-blue?style=for-the-badge)](https://s00d.github.io/vld/integrations/config-cli/config)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Donate](https://img.shields.io/badge/Donate-Donationalerts-ff4081?style=for-the-badge)](https://www.donationalerts.com/r/s00d88)

# vld-app

Validate what an application reads at its edges (config files, environment variables, CLI arguments, Tauri IPC) with [vld](https://crates.io/crates/vld) schemas.

```toml
[dependencies]
vld = "0.8"
vld-app = { version = "0.8", features = ["config-rs"] }
config = "0.15"
```

```rust
vld::schema! {
    #[derive(Debug)]
    pub struct Settings {
        pub port: i64 => vld::number().int().min(1).max(65535),
        pub host: String => vld::string().min(1),
    }
}

let cfg = config::Config::builder()
    .add_source(config::File::with_name("config.toml"))
    .build()?;
let settings: Settings = vld_app::config::from_config(&cfg)?; // every bad key reported
```

| Module | Feature | Library |
|---|---|---|
| `config` | `config-rs` and / or `figment` | config-rs 0.14 – 0.15, figment 0.10 |
| `env` | `env` and / or `dotenv` | environment variables (std), `.env` files (dotenvy 0.15) |
| `clap` | `clap` (turns on `vld/derive`) | clap 4 |
| `tauri` | — | Tauri IPC payloads (no `tauri` dependency) |

No feature is on by default; any set can be enabled together.

- Guide: <https://s00d.github.io/vld/integrations/config-cli/config>
- API: <https://docs.rs/vld-app>

## Support

If vld helps you, you can support it via [DonationAlerts](https://www.donationalerts.com/r/s00d88).
Bugs and ideas: [issues](https://github.com/s00d/vld/issues).

## License

MIT
