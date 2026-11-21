//! `vld-data` behaviour tests, one test binary: one module per integration,
//! compiled with its feature(s). Tests of a dependency supported in several
//! majors are written once (`sqlx/`, `sea/`, `redis/`, `lapin/`) and compiled
//! per major by the `sqlx08.rs` / `sqlx09.rs` / … wrappers the same way as
//! `src`. See the workspace `TESTING.md`.

// diesel's QueryableByName derive expands to `Self { id: id, .. }`, which clippy
// >= 1.99 flags as redundant_field_names; an item attribute does not reach the impl.
#![allow(clippy::redundant_field_names)]

mod story_db;
mod surrealdb;

#[cfg(all(feature = "diesel", feature = "sqlite"))]
mod diesel;
#[cfg(feature = "lapin-2")]
mod lapin2;
#[cfg(feature = "lapin-3")]
mod lapin3;
#[cfg(feature = "lapin-4")]
mod lapin4;
#[cfg(any(feature = "lapin-2", feature = "lapin-3", feature = "lapin-4"))]
#[path = "lapin/broker.rs"]
mod lapin_broker;
#[cfg(feature = "redis-0")]
mod redis0;
#[cfg(feature = "redis-1")]
mod redis1;
#[cfg(feature = "sea-orm-1")]
mod sea1;
#[cfg(feature = "sea-orm-2")]
mod sea2;
#[cfg(feature = "sqlx-0_8")]
mod sqlx08;
#[cfg(feature = "sqlx-0_9")]
mod sqlx09;
