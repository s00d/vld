use serde_json::Value;
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};

use crate::error::{value_type_name, IssueCode, VldError};
use crate::schema::VldSchema;
use md5::{Digest as _, Md5};
use sha2::Sha256;

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
    denied_media_types: Vec<String>,
    allowed_extensions: Vec<String>,
    denied_extensions: Vec<String>,
    allowed_magic_types: Vec<String>,
    denied_magic_types: Vec<String>,
    sha256_hex: Option<String>,
    md5_hex: Option<String>,
    min_width: Option<u32>,
    max_width: Option<u32>,
    min_height: Option<u32>,
    max_height: Option<u32>,
    require_exif: bool,
    storage: FileStorage,
    custom_type_error: Option<String>,
}

impl ZFile {
    pub fn new() -> Self {
        Self {
            min_size: None,
            max_size: None,
            allowed_media_types: Vec::new(),
            denied_media_types: Vec::new(),
            allowed_extensions: Vec::new(),
            denied_extensions: Vec::new(),
            allowed_magic_types: Vec::new(),
            denied_magic_types: Vec::new(),
            sha256_hex: None,
            md5_hex: None,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            require_exif: false,
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

    pub fn deny_media_type(mut self, mt: impl Into<String>) -> Self {
        self.denied_media_types.push(mt.into().to_ascii_lowercase());
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

    pub fn deny_extension(mut self, ext: impl Into<String>) -> Self {
        self.denied_extensions
            .push(ext.into().trim_start_matches('.').to_ascii_lowercase());
        self
    }

    pub fn allow_magic_type(mut self, kind: impl Into<String>) -> Self {
        self.allowed_magic_types
            .push(kind.into().to_ascii_lowercase());
        self
    }

    pub fn deny_magic_type(mut self, kind: impl Into<String>) -> Self {
        self.denied_magic_types
            .push(kind.into().to_ascii_lowercase());
        self
    }

    pub fn sha256(mut self, hex: impl Into<String>) -> Self {
        self.sha256_hex = Some(hex.into().to_ascii_lowercase());
        self
    }

    pub fn md5(mut self, hex: impl Into<String>) -> Self {
        self.md5_hex = Some(hex.into().to_ascii_lowercase());
        self
    }

    pub fn min_width(mut self, w: u32) -> Self {
        self.min_width = Some(w);
        self
    }

    pub fn max_width(mut self, w: u32) -> Self {
        self.max_width = Some(w);
        self
    }

    pub fn min_height(mut self, h: u32) -> Self {
        self.min_height = Some(h);
        self
    }

    pub fn max_height(mut self, h: u32) -> Self {
        self.max_height = Some(h);
        self
    }

    pub fn require_exif(mut self) -> Self {
        self.require_exif = true;
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
        if !self.denied_extensions.is_empty() {
            let ext = extension.as_deref().unwrap_or_default();
            if self.denied_extensions.iter().any(|deny| deny == ext) {
                return Err(VldError::single_with_value(
                    IssueCode::Custom {
                        code: "denied_file_extension".to_string(),
                    },
                    format!("File extension `{}` is denied", ext),
                    value,
                ));
            }
        }

        let needs_bytes = self.storage == FileStorage::InMemory
            || !self.allowed_media_types.is_empty()
            || !self.denied_media_types.is_empty()
            || !self.allowed_magic_types.is_empty()
            || !self.denied_magic_types.is_empty()
            || self.sha256_hex.is_some()
            || self.md5_hex.is_some()
            || self.min_width.is_some()
            || self.max_width.is_some()
            || self.min_height.is_some()
            || self.max_height.is_some()
            || self.require_exif;
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
        let magic_type = loaded_bytes
            .as_deref()
            .and_then(infer::get)
            .map(|k| k.extension().to_ascii_lowercase());
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
        if !self.denied_media_types.is_empty() {
            if let Some(mt) = media_type.as_deref() {
                if self
                    .denied_media_types
                    .iter()
                    .any(|deny| media_matches(mt, deny))
                {
                    return Err(VldError::single_with_value(
                        IssueCode::Custom {
                            code: "denied_media_type".to_string(),
                        },
                        format!("File media type `{}` is denied", mt),
                        value,
                    ));
                }
            }
        }
        if !self.allowed_magic_types.is_empty() {
            let kind = magic_type.as_deref().ok_or_else(|| {
                VldError::single_with_value(
                    IssueCode::Custom {
                        code: "unknown_magic_type".to_string(),
                    },
                    "Unable to detect file magic type",
                    value,
                )
            })?;
            if !self.allowed_magic_types.iter().any(|a| a == kind) {
                return Err(VldError::single_with_value(
                    IssueCode::Custom {
                        code: "invalid_magic_type".to_string(),
                    },
                    format!(
                        "File magic type `{}` is not allowed (expected one of: {})",
                        kind,
                        self.allowed_magic_types.join(", ")
                    ),
                    value,
                ));
            }
        }
        if !self.denied_magic_types.is_empty() {
            if let Some(kind) = magic_type.as_deref() {
                if self.denied_magic_types.iter().any(|d| d == kind) {
                    return Err(VldError::single_with_value(
                        IssueCode::Custom {
                            code: "denied_magic_type".to_string(),
                        },
                        format!("File magic type `{}` is denied", kind),
                        value,
                    ));
                }
            }
        }

        if self.sha256_hex.is_some()
            || self.md5_hex.is_some()
            || self.min_width.is_some()
            || self.max_width.is_some()
            || self.min_height.is_some()
            || self.max_height.is_some()
            || self.require_exif
        {
            let bytes = if let Some(b) = loaded_bytes.as_ref() {
                b
            } else {
                return Err(VldError::single_with_value(
                    IssueCode::IoError,
                    "Failed to load file bytes",
                    value,
                ));
            };
            if let Some(expected) = &self.sha256_hex {
                let got = format!("{:x}", Sha256::digest(bytes));
                if &got != expected {
                    return Err(VldError::single_with_value(
                        IssueCode::Custom {
                            code: "sha256_mismatch".to_string(),
                        },
                        "SHA-256 checksum mismatch",
                        value,
                    ));
                }
            }
            if let Some(expected) = &self.md5_hex {
                let got = format!("{:x}", Md5::digest(bytes));
                if &got != expected {
                    return Err(VldError::single_with_value(
                        IssueCode::Custom {
                            code: "md5_mismatch".to_string(),
                        },
                        "MD5 checksum mismatch",
                        value,
                    ));
                }
            }

            if self.min_width.is_some()
                || self.max_width.is_some()
                || self.min_height.is_some()
                || self.max_height.is_some()
            {
                let img = image::load_from_memory(bytes).map_err(|_| {
                    VldError::single_with_value(
                        IssueCode::Custom {
                            code: "invalid_image".to_string(),
                        },
                        "Unable to decode image for dimension checks",
                        value,
                    )
                })?;
                let w = img.width();
                let h = img.height();
                if let Some(min_w) = self.min_width {
                    if w < min_w {
                        return Err(VldError::single_with_value(
                            IssueCode::Custom {
                                code: "image_width_too_small".to_string(),
                            },
                            format!("Image width must be >= {}", min_w),
                            value,
                        ));
                    }
                }
                if let Some(max_w) = self.max_width {
                    if w > max_w {
                        return Err(VldError::single_with_value(
                            IssueCode::Custom {
                                code: "image_width_too_big".to_string(),
                            },
                            format!("Image width must be <= {}", max_w),
                            value,
                        ));
                    }
                }
                if let Some(min_h) = self.min_height {
                    if h < min_h {
                        return Err(VldError::single_with_value(
                            IssueCode::Custom {
                                code: "image_height_too_small".to_string(),
                            },
                            format!("Image height must be >= {}", min_h),
                            value,
                        ));
                    }
                }
                if let Some(max_h) = self.max_height {
                    if h > max_h {
                        return Err(VldError::single_with_value(
                            IssueCode::Custom {
                                code: "image_height_too_big".to_string(),
                            },
                            format!("Image height must be <= {}", max_h),
                            value,
                        ));
                    }
                }
            }

            if self.require_exif {
                let mut cur = std::io::Cursor::new(bytes);
                let exif_reader = exif::Reader::new();
                exif_reader.read_from_container(&mut cur).map_err(|_| {
                    VldError::single_with_value(
                        IssueCode::Custom {
                            code: "missing_exif".to_string(),
                        },
                        "Image must contain EXIF metadata",
                        value,
                    )
                })?;
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
