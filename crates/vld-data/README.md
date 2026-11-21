[![Crates.io](https://img.shields.io/crates/v/vld-data?style=for-the-badge)](https://crates.io/crates/vld-data)
[![Downloads](https://img.shields.io/crates/d/vld-data?style=for-the-badge)](https://crates.io/crates/vld-data)
[![docs.rs](https://img.shields.io/badge/docs-docs.rs-blue?style=for-the-badge)](https://docs.rs/vld-data)
[![Guide](https://img.shields.io/badge/guide-s00d.github.io-blue?style=for-the-badge)](https://s00d.github.io/vld/integrations/database/sqlx)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Donate](https://img.shields.io/badge/Donate-Donationalerts-ff4081?style=for-the-badge)](https://www.donationalerts.com/r/s00d88)

# vld-data

Validate values with [vld](https://crates.io/crates/vld) schemas before they are written to a database or a queue and after they are read back.

```toml
[dependencies]
vld = "0.8"
vld-data = { version = "0.8", features = ["sqlx-0_8", "sqlite"] }
```

```rust
use vld_data::sqlx08::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct NewUserSchema {
        pub name: String  => vld::string().min(1).max(100),
        pub email: String => vld::string().email(),
    }
}

#[derive(serde::Serialize)]
struct NewUser { name: String, email: String }

let user = NewUser { name: "Ada".into(), email: "not-an-email".into() };
assert!(validate_insert::<NewUserSchema, _>(&user).is_err()); // check before INSERT
```

| Module | Feature | Library |
|---|---|---|
| `sqlx08` / `sqlx09` | `sqlx-0_8` / `sqlx-0_9` + `postgres` / `mysql` / `sqlite` | SQLx 0.8 / 0.9 |
| `diesel` | `diesel` + `postgres` / `mysql` / `sqlite` | Diesel 2 |
| `sea1` / `sea2` | `sea-orm-1` / `sea-orm-2` (+ `with-*`, `postgres-array`) | SeaORM 1 / 2 |
| `surrealdb` | — | any SurrealDB SDK (through serde) |
| `redis0` / `redis1` | `redis-0` / `redis-1` (+ `redis-aio`, `redis-aio-tokio`) | redis 0.32+ / 1 |
| `lapin2` / `lapin3` / `lapin4` | `lapin-2` / `lapin-3` / `lapin-4` | lapin (RabbitMQ) 2 / 3 / 4 |
| `bson2` / `bson3` | `bson-2` / `bson-3` | MongoDB documents (bson 2 / 3) |
| `postgres_types` | `tokio-postgres` | tokio-postgres / postgres (`postgres-types` 0.2) |
| `clickhouse` | `clickhouse` | clickhouse 0.15 |
| `rumqttc` | `rumqttc` | MQTT (rumqttc 0.25) |
| `nats` | `nats` | NATS (async-nats 0.53) |
| `centrifuge` | `centrifuge` | Centrifugo client (centrifuge-client 0.1) |
| `socketio` | `socketio` | Socket.IO client (socketio-next 0.2) |
| `csv` | `csv` | CSV import (csv 1), failing rows with their line |

No feature is on by default; any set can be enabled together.

- Guide: <https://s00d.github.io/vld/integrations/database/sqlx>
- API: <https://docs.rs/vld-data>

## Support

If vld helps you, you can support it via [DonationAlerts](https://www.donationalerts.com/r/s00d88).
Bugs and ideas: [issues](https://github.com/s00d/vld/issues).

## License

MIT
