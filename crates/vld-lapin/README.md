# vld-lapin

[![crates.io](https://img.shields.io/crates/v/vld-lapin.svg)](https://crates.io/crates/vld-lapin)
[![docs.rs](https://docs.rs/vld-lapin/badge.svg)](https://docs.rs/vld-lapin)

[lapin](https://crates.io/crates/lapin) (RabbitMQ) integration for [vld](https://crates.io/crates/vld): a channel wrapper that validates message payloads before publish and after consume, with helpers that ack valid and nack / reject invalid messages.

## Install

```toml
[dependencies]
vld = "0.5"
vld-lapin = "0.5"
lapin = "2"
```

## Features

| Feature | Default | Effect |
|---|---|---|
| `lapin-2` | yes | lapin 2.x |
| `lapin-3` | | lapin 3.x |
| `lapin-4` | | lapin 4.10+ (Rust 1.88+) |

Enable exactly one; your `lapin` dependency must be the same major.

## Example

```rust
use vld_lapin::prelude::*;

vld::schema! {
    #[derive(Debug, serde::Serialize)]
    pub struct EventSchema {
        pub event: String => vld::string().min(1),
    }
}

let channel = LapinChannel::new(conn.create_channel().await?);
channel
    .publish("", "events", Default::default(), Default::default(), &EventSchema { event: "user.created".into() })
    .await?;

if let Some(msg) = channel.basic_get("events", Default::default()).await? {
    let event: EventSchema = channel
        .ack_or_nack_decode(&msg, Default::default(), Default::default())
        .await?;
}
```

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/messaging/lapin>
- API: <https://docs.rs/vld-lapin>

MSRV: 1.85 (`lapin-4`: 1.88), see [MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md).

## License

MIT
