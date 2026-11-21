//! # `vld_data::bson3` — MongoDB documents (bson 3) for `vld`
//!
//! Validate BSON documents against a `vld` schema before they are written
//! (`to_document`) and after they are read (`validate_document`,
//! `from_document`, `validate_documents`). Works with the `mongodb`
//! driver (3.x accepts bson 2 and 3) or anything else that speaks
//! `bson::Document`; this module depends on `bson` only.
//!
//! Schemas see a document as **relaxed Extended JSON**
//! (`Bson::into_relaxed_extjson`): strings, booleans, `Int32` / `Int64`
//! and finite doubles are plain JSON values, while
//!
//! - an `ObjectId` is `{"$oid": "<24 hex chars>"}`,
//! - a `DateTime` (years 1970–9999) is `{"$date": "<ISO 8601>"}` (outside
//!   that range `{"$date": {"$numberLong": "<ms>"}}`),
//! - `Decimal128`, `Binary`, NaN / ±inf doubles, … use their `$`-keyed forms.
//!
//! Map such a field to a nested schema with an `as "$oid"` / `as "$date"`
//! key (`vld::string().len(24)`, `vld::string().iso_datetime()`), as below.
//!
//! Feature `bson-3`, matching your `bson` 3 dependency; `vld_data::bson2`
//! is the same module for bson 2, and both can be enabled together.
//!
//! ```rust
//! # use ::bson3 as bson;
//! use vld_data::bson3::prelude::*;
#![doc = include_str!("bson/doctest.rs")]
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/database/mongodb>

use ::bson3 as bson;

pub use vld;

// One implementation, compiled once per bson major (ARCHITECTURE.md).
#[allow(clippy::duplicate_mod)]
#[path = "bson/shared.rs"]
mod shared;
pub use shared::*;

/// bson 3 glue: `serialize_to_document` / `deserialize_from_document`.
mod compat {
    use super::bson;

    pub(crate) fn to_document<T: serde::Serialize>(value: &T) -> Result<bson::Document, String> {
        bson::serialize_to_document(value).map_err(|e| e.to_string())
    }

    pub(crate) fn from_document<T: serde::de::DeserializeOwned>(
        doc: bson::Document,
    ) -> Result<T, String> {
        bson::deserialize_from_document(doc).map_err(|e| e.to_string())
    }
}
