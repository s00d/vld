//! # vld-data — database and message-queue integrations for `vld`
//!
//! Validate values with `vld` schemas before they are written to a database
//! or a queue and after they are read back, one module per integration.
//! Every module except `surrealdb` is behind its feature; none is on by
//! default:
//!
//! ```toml
//! vld-data = { version = "0.8", features = ["sqlx-0_8", "postgres"] }
//! ```
//!
//! | Module | Feature | What it does |
//! |---|---|---|
//! | `sqlx08` / `sqlx09` | `sqlx-0_8` / `sqlx-0_9` + `postgres` / `mysql` / `sqlite` | validate before insert / after select, column wrappers implementing `sqlx::Type` |
//! | `diesel` | `diesel` + `postgres` / `mysql` / `sqlite` | the same for Diesel 2 (`AsExpression` / `FromSqlRow` column wrappers) |
//! | `sea1` / `sea2` | `sea-orm-1` / `sea-orm-2` + `with-*`, `postgres-array` | validate SeaORM `ActiveModel`s, `impl_vld_before_save!` |
//! | [`surrealdb`] | — (no dependency) | validate SurrealDB documents through `serde`, `validate_fields!` |
//! | `redis0` / `redis1` | `redis-0` / `redis-1` (+ `redis-aio`, `redis-aio-tokio`) | a validating JSON wrapper over a redis connection |
//! | `lapin2` / `lapin3` / `lapin4` | `lapin-2` / `lapin-3` / `lapin-4` | a validating JSON wrapper over a lapin (RabbitMQ) channel |
//! | `bson2` / `bson3` | `bson-2` / `bson-3` | validate MongoDB documents (relaxed Extended JSON) before insert / after find |
//! | `postgres_types` | `tokio-postgres` | `ToSql` / `FromSql` JSON and scalar columns for tokio-postgres / postgres, validated both ways |
//! | `clickhouse` | `clickhouse` | validate rows before `Insert::write` / after `fetch_all` |
//! | `rumqttc` | `rumqttc` | validated JSON publish / decode for MQTT (rumqttc) |
//! | `nats` | `nats` | validated JSON publish / request / respond / decode for NATS (async-nats) |
//! | `centrifuge` | `centrifuge` | validated JSON publish / RPC / decode for the Centrifugo client (centrifuge-client 0.1) |
//! | `socketio` | `socketio` | validated emit / emit-with-ack / handler wrappers / acks for the Socket.IO client (socketio-next 0.1) |
//! | `csv` | `csv` | validated CSV import: every record as a vld type, failing rows with their line |
//!
//! The majors of one dependency are additive: any set of features compiles
//! together. The backend features `postgres` / `mysql` / `sqlite` apply to
//! every enabled sqlx / diesel driver; diesel's postgres and mysql backends
//! link libpq / libmysqlclient.
//!
//! ```rust
//! use vld_data::surrealdb::prelude::*;
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct NameField {
//!         pub value: String => vld::string().min(1),
//!     }
//! }
//!
//! assert!(VldText::<NameField>::new("Alice").is_ok());
//! assert!(VldText::<NameField>::new("").is_err());
//! ```
//!
//! Some features need a newer compiler than the crate's `rust-version`
//! (see MSRV.md); cargo reports the dependency's `rust-version` then.
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

pub use vld;

#[cfg(any(feature = "rumqttc", feature = "nats", feature = "centrifuge"))]
mod codec;
mod db;
mod macros;

#[cfg(feature = "bson-2")]
#[cfg_attr(docsrs, doc(cfg(feature = "bson-2")))]
pub mod bson2;
#[cfg(feature = "bson-3")]
#[cfg_attr(docsrs, doc(cfg(feature = "bson-3")))]
pub mod bson3;
#[cfg(feature = "centrifuge")]
#[cfg_attr(docsrs, doc(cfg(feature = "centrifuge")))]
pub mod centrifuge;
#[cfg(feature = "clickhouse")]
#[cfg_attr(docsrs, doc(cfg(feature = "clickhouse")))]
pub mod clickhouse;
#[cfg(feature = "csv")]
#[cfg_attr(docsrs, doc(cfg(feature = "csv")))]
pub mod csv;

#[cfg(feature = "diesel")]
#[cfg_attr(docsrs, doc(cfg(feature = "diesel")))]
pub mod diesel;
#[cfg(feature = "lapin-2")]
#[cfg_attr(docsrs, doc(cfg(feature = "lapin-2")))]
pub mod lapin2;
#[cfg(feature = "lapin-3")]
#[cfg_attr(docsrs, doc(cfg(feature = "lapin-3")))]
pub mod lapin3;
#[cfg(feature = "lapin-4")]
#[cfg_attr(docsrs, doc(cfg(feature = "lapin-4")))]
pub mod lapin4;
#[cfg(feature = "nats")]
#[cfg_attr(docsrs, doc(cfg(feature = "nats")))]
pub mod nats;
#[cfg(feature = "tokio-postgres")]
#[cfg_attr(docsrs, doc(cfg(feature = "tokio-postgres")))]
pub mod postgres_types;
#[cfg(feature = "redis-0")]
#[cfg_attr(docsrs, doc(cfg(feature = "redis-0")))]
pub mod redis0;
#[cfg(feature = "redis-1")]
#[cfg_attr(docsrs, doc(cfg(feature = "redis-1")))]
pub mod redis1;
#[cfg(feature = "rumqttc")]
#[cfg_attr(docsrs, doc(cfg(feature = "rumqttc")))]
pub mod rumqttc;
#[cfg(feature = "sea-orm-1")]
#[cfg_attr(docsrs, doc(cfg(feature = "sea-orm-1")))]
pub mod sea1;
#[cfg(feature = "sea-orm-2")]
#[cfg_attr(docsrs, doc(cfg(feature = "sea-orm-2")))]
pub mod sea2;
#[cfg(feature = "socketio")]
#[cfg_attr(docsrs, doc(cfg(feature = "socketio")))]
pub mod socketio;
#[cfg(feature = "sqlx-0_8")]
#[cfg_attr(docsrs, doc(cfg(feature = "sqlx-0_8")))]
pub mod sqlx08;
#[cfg(feature = "sqlx-0_9")]
#[cfg_attr(docsrs, doc(cfg(feature = "sqlx-0_9")))]
pub mod sqlx09;
pub mod surrealdb;
