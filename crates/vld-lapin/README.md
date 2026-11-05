[![Crates.io](https://img.shields.io/crates/v/vld-lapin?style=for-the-badge)](https://crates.io/crates/vld-lapin)
[![docs.rs](https://img.shields.io/docsrs/vld-lapin?style=for-the-badge)](https://docs.rs/vld-lapin)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=for-the-badge)](https://github.com/s00d/vld)
[![GitHub issues](https://img.shields.io/badge/github-issues-orange?style=for-the-badge)](https://github.com/s00d/vld/issues)
[![GitHub stars](https://img.shields.io/badge/github-stars-yellow?style=for-the-badge)](https://github.com/s00d/vld/stargazers)

# vld-lapin

Lapin (RabbitMQ) integration for [`vld`](https://crates.io/crates/vld).

## Version features

- `lapin-2` (default)
- `lapin-3`
- `lapin-4`

## Overview

Wrap a channel: `let channel = LapinChannel::new(channel);`
(`impl_to_lapin!(channel)` only did that rebinding and is deprecated since 0.5).

The wrapper adds:

- auto-conversion helpers: `publish`, `basic_get`, `decode_bytes`, `decode_delivery`, `decode_get`
- settle helpers (`*_get` variants take a `basic_get` message):
  - `ack_or_nack_decode` — decode + validate, **ack when valid, nack when invalid**:
    every delivery is settled (also takes `basic_get` messages)
  - `ack_decode` / `ack_decode_get` — decode + validate, **ack when valid**; an invalid
    message is left **unsettled** and the error is returned — a poison message then
    holds a prefetch slot and is redelivered (to fail again) when the channel closes
  - `nack_decode` / `nack_decode_get`, `reject_decode` / `reject_decode_get` — decode +
    validate, **nack/reject when invalid** (then return the error), so poison messages
    are not left unacked; a valid message is returned unsettled — ack it after processing
- all other native `lapin::Channel` methods remain available through deref

## Installation

```toml
[dependencies]
vld = "0.5"
vld-lapin = "0.5"
lapin = "2"
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## Quick start

```rust
use lapin::{
    options::{BasicAckOptions, BasicGetOptions, BasicPublishOptions, QueueDeclareOptions},
    types::FieldTable,
    BasicProperties, Connection, ConnectionProperties,
};
use vld_lapin::prelude::*;

vld::schema! {
    #[derive(Debug, serde::Serialize)]
    pub struct EventSchema {
        pub event: String => vld::string().min(1),
        pub retries: i64 => vld::number().int().min(0).max(5),
    }
}

let evt = EventSchema {
    event: "user.created".into(),
    retries: 0,
};

let conn = Connection::connect(
    "amqp://guest:guest@127.0.0.1:5672/%2f",
    ConnectionProperties::default(),
).await?;
let channel = LapinChannel::new(conn.create_channel().await?);

// Native method through deref.
channel
    .basic_qos(10, lapin::options::BasicQosOptions::default())
    .await?;

channel
    .queue_declare("events.user", QueueDeclareOptions::default(), FieldTable::default())
    .await?;

channel
    .publish(
        "",
        "events.user",
        BasicPublishOptions::default(),
        BasicProperties::default(),
        &evt,
    )
    .await?;

if let Some(msg) = channel.basic_get("events.user", BasicGetOptions::default()).await? {
    let parsed: EventSchema = channel.ack_decode_get(&msg, BasicAckOptions::default()).await?;
    println!("event = {}", parsed.event);
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Example

```bash
cargo run -p vld-lapin --example lapin_basic
```

## License

MIT
