[![Crates.io](https://img.shields.io/crates/v/vld-lapin?style=for-the-badge)](https://crates.io/crates/vld-lapin)
[![docs.rs](https://img.shields.io/docsrs/vld-lapin?style=for-the-badge)](https://docs.rs/vld-lapin)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=for-the-badge)](https://github.com/s00d/vld)
[![GitHub issues](https://img.shields.io/badge/github-issues-orange?style=for-the-badge)](https://github.com/s00d/vld/issues)
[![GitHub stars](https://img.shields.io/badge/github-stars-yellow?style=for-the-badge)](https://github.com/s00d/vld/stargazers)

# vld-lapin

Lapin (RabbitMQ) integration for [`vld`](https://crates.io/crates/vld).

## Overview

`vld-lapin` keeps one entrypoint:

- `impl_to_lapin!(channel)`

After rebinding, `channel` becomes a validating wrapper:

- auto-conversion helpers: `publish`, `basic_get`, `decode_bytes`, `decode_delivery`, `decode_get`
- ack helpers: `ack_decode`, `nack_decode`, `reject_decode`, `ack_decode_get`, `nack_decode_get`, `reject_decode_get`
- all other native `lapin::Channel` methods remain available through deref

## Installation

```toml
[dependencies]
vld = { version = "0.2", features = ["serialize"] }
vld-lapin = "0.2"
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
let channel = conn.create_channel().await?;
impl_to_lapin!(channel);

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
