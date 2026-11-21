# vld-config

> **Deprecated.** `vld-config` is not developed any more. Since vld 0.8 it is part of
> [`vld-app`](https://crates.io/crates/vld-app): `vld_app::config`.
>
> ```toml
> vld-app = { version = "0.8", features = ["config-rs"] }  # or "figment"
> ```
>
> Migration guide: <https://s00d.github.io/vld/guides/migration#from-07-to-08>. This version (0.7.1) is 0.7.0 unchanged, with this notice.

[![crates.io](https://img.shields.io/crates/v/vld-config.svg)](https://crates.io/crates/vld-config)
[![docs.rs](https://img.shields.io/docsrs/vld-config)](https://docs.rs/vld-config)

Validate configuration loaded with [config-rs](https://crates.io/crates/config) or [figment](https://crates.io/crates/figment) against a [vld](https://crates.io/crates/vld) schema: a typed settings struct, or an error listing every invalid key.

## Install

```toml
[dependencies]
vld = "0.5"
vld-config = "0.5"
config = "0.15"
```

## Features

| Feature | Default | Description |
|---|---|---|
| `config-rs` | yes | `from_config`, `from_builder` (config `>=0.14, <0.16`) |
| `figment` | no | `from_figment` (figment 0.10) |

The backend crates are not re-exported; add them yourself.

## Example

```rust
use vld_config::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct AppSettings {
        pub host: String => vld::string().min(1),
        pub port: i64    => vld::number().int().min(1).max(65535),
    }
}

let config = config::Config::builder()
    .add_source(config::File::with_name("config"))
    // Env values are strings: `try_parsing(true)` makes APP_PORT=8080 a number.
    .add_source(config::Environment::with_prefix("APP").try_parsing(true))
    .build()
    .unwrap();

let settings: AppSettings = from_config(&config).unwrap();
```

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/config-cli/config>
- API: <https://docs.rs/vld-config>

MSRV: 1.85, see [MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md).

## License

MIT
