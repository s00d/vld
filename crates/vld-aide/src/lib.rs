//! # vld-aide — `vld` types in aide OpenAPI documents
//!
//! [aide](https://docs.rs/aide) generates OpenAPI from `schemars::JsonSchema`.
//! This crate is an alias of [`vld_schemars`] for aide users: it re-exports
//! [`impl_json_schema!`], [`vld_to_schemars`] and [`schemars`], and has no
//! `aide` dependency of its own (add the aide version you use). Use
//! `vld-schemars` directly for the validator, introspection and `to_openapi3`.
//!
//! Features: `schemars-0` (default, schemars `>=0.9, <1`) or `schemars-1` —
//! pick the schemars major your aide version uses.
//!
//! ```rust
//! use vld::prelude::*;
//! use vld_aide::impl_json_schema;
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct User {
//!         pub name: String => vld::string().min(2).max(50),
//!         pub email: String => vld::string().email(),
//!     }
//! }
//!
//! impl_json_schema!(User);
//! // `User` implements `schemars::JsonSchema`; nested `vld::nested!` types are
//! // registered under aide's `#/components/schemas/`.
//! ```
//!
//! ```rust
//! let schema = vld_aide::vld_to_schemars(&serde_json::json!({
//!     "type": "object",
//!     "required": ["name"],
//!     "properties": { "name": { "type": "string", "minLength": 1 } }
//! }));
//! assert_eq!(schema.get("type").unwrap(), "object");
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/openapi/aide>
#![warn(missing_docs)]

pub use vld_schemars::{impl_json_schema, schemars, vld_to_schemars};

/// Prelude module — import everything you need.
pub mod prelude {
    pub use crate::impl_json_schema;
    pub use crate::vld_to_schemars;
    pub use vld::prelude::*;
}
