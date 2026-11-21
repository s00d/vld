//! schemars `>=0.9, <1`: a two-way bridge between `vld` and `schemars`.
//!
//! - **vld → schemars**: [`impl_json_schema!`] implements `schemars::JsonSchema`
//!   from a vld type's `json_schema()`; nested `vld::nested!` types are added to
//!   the generator's definitions, with `$ref`s pointing at its definitions
//!   path, and an OpenAPI 3.0 generator gets OpenAPI 3.0 schemas.
//! - **schemars → vld**: [`impl_vld_parse!`] validates a `schemars::JsonSchema`
//!   type against its schema ([`validate_with_schema`]) and implements
//!   `vld::schema::VldParse`, so it works with vld extractors.
//!
//! The selected schemars is re-exported as [`schemars`]; the
//! version-independent items of [`crate::validator`] are re-exported here.
//!
//! **aide**: aide generates OpenAPI from `schemars::JsonSchema`, so
//! `impl_json_schema!` makes a vld type usable in `aide::axum::Json<T>` and
//! friends (nested types land under aide's `#/components/schemas/`). Pick the
//! module of the schemars major your aide uses (`cargo tree -i schemars`):
//! aide 0.15 builds on schemars 0.9: aide 0.15 users use this module.
//!
//! ```rust
//! use vld_openapi::schemars0::prelude::*;
//! use vld_openapi::schemars0::schemars;
#![doc = include_str!("schemars/doctest.rs")]
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/openapi/schemars>

pub use ::schemars0 as schemars;
pub use ::vld;

// Each major compiles the shared implementation against its own alias.
#[allow(clippy::duplicate_mod)]
#[path = "schemars/shared.rs"]
mod shared;
pub use shared::*;

/// Implement `schemars::JsonSchema` for a type with a `json_schema()`
/// associated function (`vld::schema!`, `#[derive(Validate)]`):
/// `impl_json_schema!(T)` or `impl_json_schema!(T, "SchemaName")`. See the
/// [module docs](self).
#[doc(hidden)]
#[macro_export]
macro_rules! __schemars0_impl_json_schema {
    ($($t:tt)*) => { $crate::__schemars_impl_json_schema!(schemars0; $($t)*); };
}
#[doc(inline)]
pub use crate::__schemars0_impl_json_schema as impl_json_schema;

/// Implement `vld::schema::VldParse` and [`SchemarsValidate`] for a type that
/// derives `schemars::JsonSchema`, `serde::Serialize` and `serde::Deserialize`:
/// `impl_vld_parse!(T)`. The value is validated against the type's schemars
/// schema, then deserialized. See the [module docs](self).
#[doc(hidden)]
#[macro_export]
macro_rules! __schemars0_impl_vld_parse {
    ($($t:tt)*) => { $crate::__schemars_impl_vld_parse!(schemars0; $($t)*); };
}
#[doc(inline)]
pub use crate::__schemars0_impl_vld_parse as impl_vld_parse;
