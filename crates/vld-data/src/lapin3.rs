//! # `vld_data::lapin3` — lapin 3 (RabbitMQ) integration for `vld`
//!
//! Wrap a channel: `let channel = LapinChannel::new(channel);`
//!
//! The wrapper adds:
//!
//! - publish helper: `publish`
//! - get/decode helpers: `basic_get`, `decode_bytes`, `decode_delivery`, `decode_get`
//! - settle helpers: `ack_or_nack_decode` (ack when valid, nack when invalid),
//!   `ack_decode` (ack when valid — an invalid "poison" message stays unacked),
//!   `nack_decode` / `reject_decode` (nack/reject when invalid, so poison
//!   messages are not left unacked)
//! - the same for `basic_get` messages: `ack_decode_get`, `nack_decode_get`, `reject_decode_get`
//!
//! All other native `lapin::Channel` methods are still available through deref.
//!
//! Published values are `Serialize + VldParse` (a `vld::schema!` struct with
//! `#[derive(serde::Serialize)]`). Feature `lapin-3`, matching your
//! `lapin` 3 dependency; `vld_data::lapin2` / `vld_data::lapin4` are the same module for the other
//! lapin majors, and any of them can be enabled together.
//!
//! ```no_run
//! use vld_data::lapin3::prelude::*;
#![doc = include_str!("lapin/doctest.rs")]
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/messaging/lapin>

use ::lapin3 as lapin;

pub use vld;

// One implementation, compiled once per lapin major (ARCHITECTURE.md).
#[allow(clippy::duplicate_mod)]
#[path = "lapin/shared.rs"]
mod shared;
pub use shared::*;

/// lapin 3 glue.
mod compat {
    use super::lapin;

    /// What `LapinChannel::publish` returns: the publisher confirmation.
    pub(crate) type LapinConfirmation = lapin::publisher_confirm::Confirmation;
}
