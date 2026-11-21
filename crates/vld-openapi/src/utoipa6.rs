//! utoipa 6: `vld` schemas as utoipa OpenAPI types.
//!
//! [`impl_to_schema!`] implements utoipa's `PartialSchema`, `ToSchema` and
//! `IntoParams` from the vld `json_schema()` of a `vld::schema!` or
//! `#[derive(Validate)]` type, so validation rules and OpenAPI docs come from
//! one definition. Nested `vld::nested!` types are registered as components
//! automatically. [`json_schema_to_schema`] converts any JSON Schema value.
//! utoipa 6 is re-exported as [`utoipa`].
//!
//! Mark parameter structs with utoipa's `#[into_params(parameter_in = Query)]`
//! (or `Path`, `Header`, `Cookie`), or pass the location to the macro:
//! `impl_to_schema!(T, query)`. `impl_to_schema!(T, "Name")` sets the
//! component name.
//!
//! ```rust
//! use vld_openapi::utoipa6::prelude::*;
//! use vld_openapi::utoipa6::utoipa;
#![doc = include_str!("utoipa/doctest.rs")]
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/openapi/utoipa>

pub use ::utoipa6 as utoipa;

// Each major compiles the shared implementation against its own alias.
#[allow(clippy::duplicate_mod)]
#[path = "utoipa/shared.rs"]
mod shared;
pub use shared::*;

/// Bridge a vld type to utoipa `ToSchema` + `IntoParams`: `impl_to_schema!(T)`,
/// `impl_to_schema!(T, "ComponentName")`, or `impl_to_schema!(T, query)`
/// (`path`, `header`, `cookie`) to force the parameter location. See the
/// [module docs](self).
#[doc(hidden)]
#[macro_export]
macro_rules! __utoipa6_impl_to_schema {
    ($($t:tt)*) => { $crate::__utoipa_impl_to_schema!(utoipa6; $($t)*); };
}
#[doc(inline)]
pub use crate::__utoipa6_impl_to_schema as impl_to_schema;
