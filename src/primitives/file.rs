use serde_json::Value;
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};

use crate::error::{value_type_name, IssueCode, VldError};
use crate::schema::VldSchema;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFile {
    path: PathBuf,
    size: u64,
    media_type: Option<String>,
    extension: Option<String>,
    bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStorage {
    PathOnly,
    InMemory,
}

impl ValidatedFile {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn media_type(&self) -> Option<&str> {
        self.media_type.as_deref()
    }

    pub fn extension(&self) -> Option<&str> {
        self.extension.as_deref()
    }

    pub fn storage(&self) -> FileStorage {
        if self.bytes.is_some() {
            FileStorage::InMemory
        } else {
            FileStorage::PathOnly
        }
    }

    pub fn bytes(&self) -> Option<&[u8]> {
        self.bytes.as_deref()
    }

    pub fn into_bytes(self) -> Option<Vec<u8>> {
        self.bytes
    }

    pub fn read_bytes(&self) -> Result<Vec<u8>, VldError> {
        if let Some(bytes) = &self.bytes {
            return Ok(bytes.clone());
        }
        fs::read(&self.path).map_err(|e| {
            VldError::single(
                IssueCode::IoError,
                format!("Failed to read file {}: {}", self.path.display(), e),
            )
        })
    }

    pub fn open(&self) -> Result<File, VldError> {
        File::open(&self.path).map_err(|e| {
            VldError::single(
                IssueCode::IoError,
                format!("Failed to open file {}: {}", self.path.display(), e),
            )
        })
    }
}

#[derive(Clone)]
pub struct ZFile {
    min_size: Option<u64>,
    max_size: Option<u64>,
    allowed_media_types: Vec<String>,
    allowed_extensions: Vec<String>,
    storage: FileStorage,
    custom_type_error: Option<String>,
}

impl ZFile {
    pub fn new() -> Self {
        Self {
            min_size: None,
            max_size: None,
            allowed_media_types: Vec::new(),
            allowed_extensions: Vec::new(),
            storage: FileStorage::InMemory,
            custom_type_error: None,
        }
    }

    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    pub fn min_size(mut self, bytes: u64) -> Self {
        self.min_size = Some(bytes);
        self
    }

    pub fn max_size(mut self, bytes: u64) -> Self {
        self.max_size = Some(bytes);
        self
    }

    pub fn non_empty(self) -> Self {
        self.min_size(1)
    }

    pub fn media_type(mut self, mt: impl Into<String>) -> Self {
        self.allowed_media_types
            .push(mt.into().to_ascii_lowercase());
        self
    }

    pub fn media_types(mut self, types: &[&str]) -> Self {
        self.allowed_media_types
            .extend(types.iter().map(|s| s.to_ascii_lowercase()));
        self
    }

    pub fn extension(mut self, ext: impl Into<String>) -> Self {
        let normalized = ext.into().trim_start_matches('.').to_ascii_lowercase();
        self.allowed_extensions.push(normalized);
        self
    }

    pub fn extensions(mut self, exts: &[&str]) -> Self {
        self.allowed_extensions.extend(
            exts.iter()
                .map(|s| s.trim_start_matches('.').to_ascii_lowercase()),
        );
        self
    }

    /// Store only file path and metadata in output.
    ///
    /// Use [`ValidatedFile::read_bytes`] / [`ValidatedFile::open`] to get content later.
    pub fn store_path_only(mut self) -> Self {
        self.storage = FileStorage::PathOnly;
        self
    }

    /// Store full file bytes in output (default mode).
    pub fn store_in_memory(mut self) -> Self {
        self.storage = FileStorage::InMemory;
        self
    }

    pub fn parse_path(&self, path: impl AsRef<Path>) -> Result<ValidatedFile, VldError> {
        self.parse_value(&Value::String(path.as_ref().to_string_lossy().to_string()))
    }

    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        let mut schema = serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "size": { "type": "integer", "minimum": 0 },
                "mediaType": { "type": ["string", "null"] }
            },
            "required": ["path", "size"]
        });
        if let Some(min) = self.min_size {
            schema["properties"]["size"]["minimum"] = serde_json::json!(min);
        }
        if let Some(max) = self.max_size {
            schema["properties"]["size"]["maximum"] = serde_json::json!(max);
        }
        schema
    }
}

impl Default for ZFile {
    fn default() -> Self {
        Self::new()
    }
}

fn media_matches(actual: &str, rule: &str) -> bool {
    if let Some(prefix) = rule.strip_suffix("/*") {
        actual.starts_with(&(prefix.to_string() + "/"))
    } else {
        actual == rule
    }
}

fn extract_path(value: &Value) -> Result<&str, VldError> {
    match value {
        Value::String(s) => Ok(s.as_str()),
        Value::Object(obj) => obj.get("path").and_then(Value::as_str).ok_or_else(|| {
            VldError::single_with_value(
                IssueCode::InvalidType {
                    expected: "string or object with {path: string}".to_string(),
                    received: value_type_name(value),
                },
                "Expected file path string or object with `path` field",
                value,
            )
        }),
        _ => Err(VldError::single_with_value(
            IssueCode::InvalidType {
                expected: "string or object with {path: string}".to_string(),
                received: value_type_name(value),
            },
            "Expected file path string or object with `path` field",
            value,
        )),
    }
}

impl VldSchema for ZFile {
    type Output = ValidatedFile;

    fn parse_value(&self, value: &Value) -> Result<ValidatedFile, VldError> {
        let path_str = extract_path(value).map_err(|e| {
            if let Some(msg) = &self.custom_type_error {
                VldError::single_with_value(
                    IssueCode::InvalidType {
                        expected: "file path".to_string(),
                        received: value_type_name(value),
                    },
                    msg.clone(),
                    value,
                )
            } else {
                e
            }
        })?;
        let path = PathBuf::from(path_str);

        if !path.exists() {
            return Err(VldError::single_with_value(
                IssueCode::IoError,
                format!("File does not exist: {}", path.display()),
                value,
            ));
        }
        if !path.is_file() {
            return Err(VldError::single_with_value(
                IssueCode::IoError,
                format!("Path is not a file: {}", path.display()),
                value,
            ));
        }

        let metadata = fs::metadata(&path).map_err(|e| {
            VldError::single_with_value(
                IssueCode::IoError,
                format!("Failed to read file metadata: {}", e),
                value,
            )
        })?;
        let size = metadata.len();

        if let Some(min) = self.min_size {
            if size < min {
                return Err(VldError::single_with_value(
                    IssueCode::TooSmall {
                        minimum: min as f64,
                        inclusive: true,
                    },
                    format!("File size must be at least {} bytes", min),
                    value,
                ));
            }
        }
        if let Some(max) = self.max_size {
            if size > max {
                return Err(VldError::single_with_value(
                    IssueCode::TooBig {
                        maximum: max as f64,
                        inclusive: true,
                    },
                    format!("File size must be at most {} bytes", max),
                    value,
                ));
            }
        }

        let extension = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase());
        if !self.allowed_extensions.is_empty() {
            let ext = extension.as_deref().unwrap_or_default();
            if !self.allowed_extensions.iter().any(|allowed| allowed == ext) {
                return Err(VldError::single_with_value(
                    IssueCode::Custom {
                        code: "invalid_file_extension".to_string(),
                    },
                    format!(
                        "File extension must be one of: {}",
                        self.allowed_extensions.join(", ")
                    ),
                    value,
                ));
            }
        }

        let needs_bytes =
            self.storage == FileStorage::InMemory || !self.allowed_media_types.is_empty();
        let loaded_bytes = if needs_bytes {
            Some(fs::read(&path).map_err(|e| {
                VldError::single_with_value(
                    IssueCode::IoError,
                    format!("Failed to read file: {}", e),
                    value,
                )
            })?)
        } else {
            None
        };

        let media_type = loaded_bytes
            .as_deref()
            .and_then(infer::get)
            .map(|k| k.mime_type().to_ascii_lowercase());
        if !self.allowed_media_types.is_empty() {
            let mt = media_type.as_deref().ok_or_else(|| {
                VldError::single_with_value(
                    IssueCode::Custom {
                        code: "unknown_media_type".to_string(),
                    },
                    "Unable to detect file media type",
                    value,
                )
            })?;
            if !self
                .allowed_media_types
                .iter()
                .any(|allowed| media_matches(mt, allowed))
            {
                return Err(VldError::single_with_value(
                    IssueCode::Custom {
                        code: "invalid_media_type".to_string(),
                    },
                    format!(
                        "File media type `{}` is not allowed (expected one of: {})",
                        mt,
                        self.allowed_media_types.join(", ")
                    ),
                    value,
                ));
            }
        }

        Ok(ValidatedFile {
            path,
            size,
            media_type,
            extension,
            bytes: if self.storage == FileStorage::InMemory {
                loaded_bytes
            } else {
                None
            },
        })
    }
}
