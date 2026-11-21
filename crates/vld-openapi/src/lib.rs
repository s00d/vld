//! # vld-openapi — `vld` schemas in OpenAPI tools
//!
//! One module per integration, each behind its feature (none is on by
//! default). The majors of one dependency are additive: any set of features
//! compiles together.
//!
//! | module | feature | what it does |
//! |---|---|---|
//! | [`utoipa5`] / [`utoipa6`] | `utoipa-5` / `utoipa-6` | `impl_to_schema!`: utoipa `ToSchema` / `IntoParams` from a vld type |
//! | [`schemars0`] / [`schemars1`] | `schemars-0` / `schemars-1` | `impl_json_schema!` (vld → `schemars::JsonSchema`, also for aide) and `impl_vld_parse!` (schemars type → vld validation) |
//! | [`validator`] | always | validate JSON against a JSON Schema, inspect and combine schemas, `to_openapi3` |
//!
//! ```toml
//! vld-openapi = { version = "0.8", features = ["utoipa-5"] }
//! ```
//!
//! ```rust
//! let schema = serde_json::json!({"type": "string", "minLength": 2});
//! assert!(vld_openapi::validator::validate_with_schema(&schema, &"hi".into()).is_ok());
//! assert!(vld_openapi::validator::validate_with_schema(&schema, &"h".into()).is_err());
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/openapi/utoipa>
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

pub use vld;

mod macros;

pub mod validator;

#[cfg(feature = "schemars-0")]
#[cfg_attr(docsrs, doc(cfg(feature = "schemars-0")))]
pub mod schemars0;
#[cfg(feature = "schemars-1")]
#[cfg_attr(docsrs, doc(cfg(feature = "schemars-1")))]
pub mod schemars1;
#[cfg(feature = "utoipa-5")]
#[cfg_attr(docsrs, doc(cfg(feature = "utoipa-5")))]
pub mod utoipa5;
#[cfg(feature = "utoipa-6")]
#[cfg_attr(docsrs, doc(cfg(feature = "utoipa-6")))]
pub mod utoipa6;

/// Fallback for types without nested vld schemas: the inherent
/// `__vld_nested_schemas` of `vld::schema!` / derive types wins over this
/// trait method in `<T>::f()` path resolution.
#[doc(hidden)]
pub trait __VldNestedSchemasFallback {
    fn __vld_nested_schemas() -> Vec<vld::openapi::NestedSchemaEntry> {
        Vec::new()
    }
}

impl<T: ?Sized> __VldNestedSchemasFallback for T {}
