//! # `vld_data::sqlx08` — SQLx 0.8 integration for `vld`
//!
//! Validate values against a [`vld`] schema before writing them with SQLx
//! ([`validate_insert`], [`Validated`]) or after loading them
//! ([`validate_row`], [`validate_rows`]), and bind validated column wrappers
//! ([`VldText`], [`VldInt`], [`VldFloat`], [`VldBool`]) that implement
//! `sqlx::Type`, `Encode` and `Decode`.
//!
//! Feature `sqlx-0_8` plus the backends you use: `sqlite`, `postgres`,
//! `mysql`. `vld_data::sqlx09` is the same module for SQLx 0.9
//! (feature `sqlx-0_9`); both can be enabled together.
//!
//! ```
//! use vld_data::sqlx08::prelude::*;
#![doc = include_str!("sqlx/doctest.rs")]
//! ```
//!
//! Decoding does not re-validate (data from the database is trusted); call
//! [`validate_row`] to re-check loaded rows.
//!
//! Guide: <https://s00d.github.io/vld/integrations/database/sqlx>

use ::sqlx08 as sqlx;

pub use vld;

// One implementation, compiled once per SQLx major (ARCHITECTURE.md).
#[allow(clippy::duplicate_mod)]
#[path = "sqlx/shared.rs"]
mod shared;
pub use shared::*;

/// SQLx 0.8 glue.
mod compat {
    use super::sqlx;

    /// The encode buffer type of `DB`.
    pub(crate) type SqlxArgBuf<'q, DB> = <DB as sqlx::Database>::ArgumentBuffer<'q>;
}
