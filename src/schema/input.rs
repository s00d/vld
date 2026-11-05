use serde_json::Value;
use std::borrow::Cow;

use crate::error::{IssueCode, VldError};

/// Trait for types that can be used as input to schema parsing.
///
/// Implemented for JSON strings (`str`, `String`), raw bytes (`[u8]`), file
/// paths (`Path`, `PathBuf`) and `serde_json::Value` (borrowed, never cloned).
pub trait VldInput {
    /// The input as a JSON value: borrowed when it already is one, parsed otherwise.
    fn to_json_value(&self) -> Result<Cow<'_, Value>, VldError>;
}

impl VldInput for Value {
    fn to_json_value(&self) -> Result<Cow<'_, Value>, VldError> {
        Ok(Cow::Borrowed(self))
    }
}

fn invalid_json(e: serde_json::Error) -> VldError {
    VldError::single(IssueCode::ParseError, format!("Invalid JSON: {}", e))
}

impl VldInput for str {
    fn to_json_value(&self) -> Result<Cow<'_, Value>, VldError> {
        serde_json::from_str(self)
            .map(Cow::Owned)
            .map_err(invalid_json)
    }
}

impl VldInput for String {
    fn to_json_value(&self) -> Result<Cow<'_, Value>, VldError> {
        self.as_str().to_json_value()
    }
}

impl VldInput for [u8] {
    fn to_json_value(&self) -> Result<Cow<'_, Value>, VldError> {
        serde_json::from_slice(self)
            .map(Cow::Owned)
            .map_err(invalid_json)
    }
}

impl VldInput for std::path::Path {
    fn to_json_value(&self) -> Result<Cow<'_, Value>, VldError> {
        let content = std::fs::read_to_string(self).map_err(|e| {
            VldError::single(IssueCode::IoError, format!("Failed to read file: {}", e))
        })?;
        serde_json::from_str(&content)
            .map(Cow::Owned)
            .map_err(invalid_json)
    }
}

impl VldInput for std::path::PathBuf {
    fn to_json_value(&self) -> Result<Cow<'_, Value>, VldError> {
        self.as_path().to_json_value()
    }
}
