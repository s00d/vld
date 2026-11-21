//! # `vld_data::sea1` — SeaORM 1 integration for `vld`
//!
//! Validate [`ActiveModel`](sea_orm::ActiveModelTrait) fields against a
//! [`vld`] schema **before** `insert()` / `update()` reaches the database:
//!
//! - [`validate_active`] — turns the `Set` / `Unchanged` values into a JSON
//!   object (`NotSet` fields are omitted) and runs the schema;
//! - [`validate_model`] / [`validate_json`] — any `Serialize` value (an input
//!   DTO, a `Model`) or raw JSON;
//! - [`before_save`] / [`impl_vld_before_save!`] — validation inside
//!   [`ActiveModelBehavior::before_save`](sea_orm::ActiveModelBehavior::before_save).
//!
//! Feature `sea-orm-1`; SeaORM 1 is re-exported as [`sea_orm`].
//! `vld_data::sea2` is the same module for SeaORM 2 (feature
//! `sea-orm-2`); both can be enabled together. `with-json`,
//! `with-chrono`, `with-time`, `with-uuid`, `with-rust_decimal`,
//! `with-bigdecimal`, `postgres-array` map those column types to JSON
//! (without them such columns are validated as `null`).
//!
//! ```
//! use vld_data::sea1::prelude::*;
#![doc = include_str!("sea/doctest.rs")]
//! ```
//!
//! With an entity:
//!
//! ```rust,ignore
//! vld_data::sea1::validate_active::<UserInput, _>(&active_model)?;
//! // or, in the entity module, on every insert / update:
//! vld_data::sea1::impl_vld_before_save!(ActiveModel, super::UserInput);
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/database/sea-orm>

/// The SeaORM 1 crate this module is built on.
pub use ::sea_orm1 as sea_orm;

pub use vld;

// One implementation, compiled once per SeaORM major (ARCHITECTURE.md).
#[allow(clippy::duplicate_mod)]
#[path = "sea/shared.rs"]
mod shared;
pub use shared::*;

/// Implements [`ActiveModelBehavior`](sea_orm::ActiveModelBehavior) with
/// automatic vld validation in `before_save`.
///
/// ```rust,ignore
/// // In your entity module:
/// vld_data::sea1::impl_vld_before_save!(ActiveModel, UserInput);
/// // Different schemas for insert and update:
/// vld_data::sea1::impl_vld_before_save!(ActiveModel, insert: NewUser, update: UserPatch);
/// ```
///
/// This expands to an `ActiveModelBehavior` impl that calls
/// [`validate_active`] before every insert/update.
#[doc(hidden)]
#[macro_export]
macro_rules! __sea1_impl_vld_before_save {
    ($($t:tt)*) => { $crate::__sea_impl_vld_before_save!(sea1; $($t)*); };
}
#[doc(inline)]
pub use crate::__sea1_impl_vld_before_save as impl_vld_before_save;

/// SeaORM 1 glue.
mod compat {
    /// `None`s and the variants without a vld-data feature.
    pub(crate) fn other_value_to_json(_: super::sea_orm::Value) -> serde_json::Value {
        serde_json::Value::Null
    }
}
