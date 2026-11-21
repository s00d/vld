//! The bson integration, written once against `super::bson` and compiled by
//! `bson2.rs` / `bson3.rs`; the function names that differ between the
//! majors are in their `compat` modules.

use super::bson::{Bson, Document};
use super::compat;
use std::fmt;

/// The relaxed Extended JSON of `doc`: what the schemas see.
fn to_json(doc: &Document) -> serde_json::Value {
    Bson::Document(doc.clone()).into_relaxed_extjson()
}

/// Validate a BSON document against schema `S` and return the parsed schema
/// value.
///
/// The document is checked as relaxed Extended JSON: an `ObjectId` is
/// `{"$oid": "<24 hex>"}`, a date `{"$date": "<ISO 8601>"}`, an `Int64` a
/// plain number (see the module docs).
pub fn validate_document<S>(doc: &Document) -> Result<S, VldBsonError>
where
    S: vld::schema::VldParse,
{
    S::vld_parse_value(&to_json(doc)).map_err(VldBsonError::Validation)
}

/// Validate a batch of documents against schema `S`.
///
/// Returns the index and error of the first invalid document.
pub fn validate_documents<S>(docs: &[Document]) -> Result<(), (usize, VldBsonError)>
where
    S: vld::schema::VldParse,
{
    for (i, doc) in docs.iter().enumerate() {
        validate_document::<S>(doc).map_err(|e| (i, e))?;
    }
    Ok(())
}

/// Serialize `value` to a BSON document and validate the document against
/// schema `S` — use the result for `insert_one` / `replace_one`.
///
/// The document is validated, not `value`, so the schema sees what MongoDB
/// stores (`ObjectId` fields as `{"$oid": ..}`).
pub fn to_document<S, T>(value: &T) -> Result<Document, VldBsonError>
where
    S: vld::schema::VldParse,
    T: serde::Serialize,
{
    let doc = compat::to_document(value).map_err(VldBsonError::Serialization)?;
    validate_document::<S>(&doc)?;
    Ok(doc)
}

/// Validate a document read from MongoDB against schema `S`, then
/// deserialize it into `T`.
pub fn from_document<S, T>(doc: Document) -> Result<T, VldBsonError>
where
    S: vld::schema::VldParse,
    T: serde::de::DeserializeOwned,
{
    validate_document::<S>(&doc)?;
    compat::from_document(doc).map_err(VldBsonError::Deserialization)
}

/// Error type of the bson module.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum VldBsonError {
    /// The document failed schema validation.
    Validation(vld::error::VldError),
    /// The value could not be serialized to a BSON document.
    Serialization(String),
    /// The document could not be deserialized into the target type.
    Deserialization(String),
}

impl fmt::Display for VldBsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VldBsonError::Validation(e) => write!(f, "Validation error: {e}"),
            VldBsonError::Serialization(e) => write!(f, "Serialization error: {e}"),
            VldBsonError::Deserialization(e) => write!(f, "Deserialization error: {e}"),
        }
    }
}

impl std::error::Error for VldBsonError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VldBsonError::Validation(e) => Some(e),
            VldBsonError::Serialization(_) | VldBsonError::Deserialization(_) => None,
        }
    }
}

impl From<vld::error::VldError> for VldBsonError {
    fn from(e: vld::error::VldError) -> Self {
        Self::Validation(e)
    }
}

/// Common imports: the functions, `VldBsonError` and `vld::prelude`.
pub mod prelude {
    pub use super::{
        VldBsonError, from_document, to_document, validate_document, validate_documents,
    };
    pub use vld::prelude::*;
}
