use serde_json::Value;

use crate::error::{IssueCode, VldError};

/// Trait for types that can be used as input to schema parsing.
///
/// Implemented for JSON strings (`&str`, `String`), raw bytes (`&[u8]`),
/// file paths (`Path`, `PathBuf` — requires the `std` feature), and
/// `serde_json::Value`.
pub trait VldInput {
    /// Convert this input into a `serde_json::Value`.
    fn to_json_value(&self) -> Result<Value, VldError>;
}

impl VldInput for Value {
    fn to_json_value(&self) -> Result<Value, VldError> {
        Ok(self.clone())
    }
}

impl VldInput for str {
    fn to_json_value(&self) -> Result<Value, VldError> {
        serde_json::from_str(self)
            .map_err(|e| VldError::single(IssueCode::ParseError, format!("Invalid JSON: {}", e)))
    }
}

impl VldInput for String {
    fn to_json_value(&self) -> Result<Value, VldError> {
        self.as_str().to_json_value()
    }
}

impl VldInput for [u8] {
    fn to_json_value(&self) -> Result<Value, VldError> {
        serde_json::from_slice(self)
            .map_err(|e| VldError::single(IssueCode::ParseError, format!("Invalid JSON: {}", e)))
    }
}

#[cfg(feature = "std")]
impl VldInput for std::path::Path {
    fn to_json_value(&self) -> Result<Value, VldError> {
        let content = std::fs::read_to_string(self).map_err(|e| {
            VldError::single(IssueCode::IoError, format!("Failed to read file: {}", e))
        })?;
        content.as_str().to_json_value()
    }
}

#[cfg(feature = "std")]
impl VldInput for std::path::PathBuf {
    fn to_json_value(&self) -> Result<Value, VldError> {
        self.as_path().to_json_value()
    }
}
