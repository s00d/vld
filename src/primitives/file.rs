use serde_json::Value;
use std::fs;
use std::fs::File;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use crate::error::{value_type_name, FileTypeKind, IssueCode, SizeOrigin, VldError};
use crate::primitives::path::{outside_base_error, resolve_within};
use crate::schema::VldSchema;
#[cfg(feature = "file-advanced")]
use md5::{Digest as _, Md5};
#[cfg(feature = "file-advanced")]
use sha2::Sha256;

#[cfg(feature = "file-advanced")]
fn digest_to_hex(output: impl AsRef<[u8]>) -> String {
    output
        .as_ref()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

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
    within_base: Option<PathBuf>,
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
            within_base: None,
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

    #[cfg(feature = "file-advanced")]
    pub fn sha256(mut self, hex: impl Into<String>) -> Self {
        self.sha256_hex = Some(hex.into().to_ascii_lowercase());
        self
    }

    #[cfg(feature = "file-advanced")]
    pub fn md5(mut self, hex: impl Into<String>) -> Self {
        self.md5_hex = Some(hex.into().to_ascii_lowercase());
        self
    }

    #[cfg(feature = "file-advanced")]
    pub fn min_width(mut self, w: u32) -> Self {
        self.min_width = Some(w);
        self
    }

    #[cfg(feature = "file-advanced")]
    pub fn max_width(mut self, w: u32) -> Self {
        self.max_width = Some(w);
        self
    }

    #[cfg(feature = "file-advanced")]
    pub fn min_height(mut self, h: u32) -> Self {
        self.min_height = Some(h);
        self
    }

    #[cfg(feature = "file-advanced")]
    pub fn max_height(mut self, h: u32) -> Self {
        self.max_height = Some(h);
        self
    }

    #[cfg(feature = "file-advanced")]
    pub fn require_exif(mut self) -> Self {
        self.require_exif = true;
        self
    }

    /// Require the file to live inside `base` (same rules as
    /// [`ZPath::within`](crate::primitives::ZPath::within)): relative inputs are
    /// resolved against `base`, `..` and symlinks cannot escape it, and the
    /// returned [`ValidatedFile::path`] is the joined path.
    ///
    /// **Use this whenever the path comes from untrusted input** — without it
    /// any readable file (e.g. `{"path": "/etc/passwd"}`) is accepted.
    pub fn within(mut self, base: impl Into<PathBuf>) -> Self {
        self.within_base = Some(base.into());
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
            VldError::from_code_with_value(
                IssueCode::InvalidType {
                    expected: "file".to_string(),
                    received: value_type_name(value),
                },
                value,
            )
        }),
        _ => Err(VldError::from_code_with_value(
            IssueCode::InvalidType {
                expected: "file".to_string(),
                received: value_type_name(value),
            },
            value,
        )),
    }
}

fn file_err(code: IssueCode, value: &Value) -> VldError {
    VldError::from_code_with_value(code, value)
}

fn not_allowed(kind: FileTypeKind, actual: &str, allowed: &[String], value: &Value) -> VldError {
    file_err(
        IssueCode::FileTypeNotAllowed {
            kind,
            actual: actual.to_string(),
            allowed: allowed.to_vec(),
        },
        value,
    )
}

fn denied(kind: FileTypeKind, actual: &str, value: &Value) -> VldError {
    file_err(
        IssueCode::FileTypeDenied {
            kind,
            actual: actual.to_string(),
        },
        value,
    )
}

#[cfg(feature = "file-advanced")]
fn checksum_mismatch(algorithm: &str, value: &Value) -> VldError {
    file_err(
        IssueCode::ChecksumMismatch {
            algorithm: algorithm.to_string(),
        },
        value,
    )
}

fn io_err(msg: impl Into<String>, value: &Value) -> VldError {
    VldError::single_with_value(IssueCode::IoError, msg, value)
}

impl ZFile {
    /// Checks that need the file content (hashes, image dimensions, EXIF).
    fn needs_content_checks(&self) -> bool {
        self.sha256_hex.is_some()
            || self.md5_hex.is_some()
            || self.needs_dimensions()
            || self.require_exif
    }

    fn needs_dimensions(&self) -> bool {
        self.min_width.is_some()
            || self.max_width.is_some()
            || self.min_height.is_some()
            || self.max_height.is_some()
    }

    fn needs_bytes(&self) -> bool {
        self.storage == FileStorage::InMemory
            || !self.allowed_media_types.is_empty()
            || !self.denied_media_types.is_empty()
            || !self.allowed_magic_types.is_empty()
            || !self.denied_magic_types.is_empty()
            || self.needs_content_checks()
    }

    fn check_size(&self, size: u64, value: &Value) -> Result<(), VldError> {
        if let Some(min) = self.min_size.filter(|&m| size < m) {
            return Err(VldError::from_code_with_value(
                IssueCode::TooSmall {
                    origin: SizeOrigin::File,
                    minimum: min.into(),
                    inclusive: true,
                },
                value,
            ));
        }
        if let Some(max) = self.max_size.filter(|&m| size > m) {
            return Err(VldError::from_code_with_value(
                IssueCode::TooBig {
                    origin: SizeOrigin::File,
                    maximum: max.into(),
                    inclusive: true,
                },
                value,
            ));
        }
        Ok(())
    }

    #[cfg(feature = "file-advanced")]
    fn check_dimensions(&self, bytes: &[u8], value: &Value) -> Result<(), VldError> {
        // Reads only the image header; the pixels are never decoded.
        let (w, h) = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .ok()
            .and_then(|r| r.into_dimensions().ok())
            .ok_or_else(|| {
                file_err(
                    IssueCode::InvalidFormat {
                        format: crate::error::FormatKind::Image,
                    },
                    value,
                )
            })?;
        let small = |origin, bound: u32| IssueCode::TooSmall {
            origin,
            minimum: bound.into(),
            inclusive: true,
        };
        let big = |origin, bound: u32| IssueCode::TooBig {
            origin,
            maximum: bound.into(),
            inclusive: true,
        };
        let failure = [
            self.min_width
                .filter(|&m| w < m)
                .map(|m| small(SizeOrigin::ImageWidth, m)),
            self.max_width
                .filter(|&m| w > m)
                .map(|m| big(SizeOrigin::ImageWidth, m)),
            self.min_height
                .filter(|&m| h < m)
                .map(|m| small(SizeOrigin::ImageHeight, m)),
            self.max_height
                .filter(|&m| h > m)
                .map(|m| big(SizeOrigin::ImageHeight, m)),
        ]
        .into_iter()
        .flatten()
        .next();
        match failure {
            Some(code) => Err(file_err(code, value)),
            None => Ok(()),
        }
    }
}

impl VldSchema for ZFile {
    type Output = ValidatedFile;

    fn parse_value(&self, value: &Value) -> Result<ValidatedFile, VldError> {
        let path_str = extract_path(value).map_err(|e| {
            if self.custom_type_error.is_some() {
                crate::primitives::type_error(&self.custom_type_error, "file", value)
            } else {
                e
            }
        })?;
        let mut path = PathBuf::from(path_str);
        if let Some(base) = &self.within_base {
            path = resolve_within(base, &path).ok_or_else(|| outside_base_error(base, value))?;
        }

        if !path.exists() {
            return Err(file_err(IssueCode::PathNotFound, value));
        }
        if !path.is_file() {
            return Err(file_err(IssueCode::PathNotFile, value));
        }

        let extension = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase());
        let ext = extension.as_deref().unwrap_or_default();
        if !self.allowed_extensions.is_empty()
            && !self.allowed_extensions.iter().any(|allowed| allowed == ext)
        {
            return Err(not_allowed(
                FileTypeKind::Extension,
                ext,
                &self.allowed_extensions,
                value,
            ));
        }
        if self.denied_extensions.iter().any(|deny| deny == ext) {
            return Err(denied(FileTypeKind::Extension, ext, value));
        }

        // Size and content come from the same open handle (no stat/read race),
        // and reading never goes past `max_size + 1` bytes.
        let file =
            File::open(&path).map_err(|e| io_err(format!("Failed to open file: {}", e), value))?;
        let mut size = file
            .metadata()
            .map_err(|e| io_err(format!("Failed to read file metadata: {}", e), value))?
            .len();
        self.check_size(size, value)?;

        let loaded_bytes = if self.needs_bytes() {
            let mut buf = Vec::new();
            let limit = self.max_size.map_or(u64::MAX, |m| m.saturating_add(1));
            file.take(limit)
                .read_to_end(&mut buf)
                .map_err(|e| io_err(format!("Failed to read file: {}", e), value))?;
            size = buf.len() as u64;
            self.check_size(size, value)?;
            Some(buf)
        } else {
            None
        };

        let kind = loaded_bytes.as_deref().and_then(infer::get);
        let media_type = kind.map(|k| k.mime_type().to_ascii_lowercase());
        let magic_type = kind.map(|k| k.extension().to_ascii_lowercase());

        if !self.allowed_media_types.is_empty() {
            let mt = media_type.as_deref().ok_or_else(|| {
                file_err(
                    IssueCode::FileTypeUnknown {
                        kind: FileTypeKind::MediaType,
                    },
                    value,
                )
            })?;
            if !self
                .allowed_media_types
                .iter()
                .any(|allowed| media_matches(mt, allowed))
            {
                return Err(not_allowed(
                    FileTypeKind::MediaType,
                    mt,
                    &self.allowed_media_types,
                    value,
                ));
            }
        }
        if let Some(mt) = media_type.as_deref() {
            if self
                .denied_media_types
                .iter()
                .any(|deny| media_matches(mt, deny))
            {
                return Err(denied(FileTypeKind::MediaType, mt, value));
            }
        }
        if !self.allowed_magic_types.is_empty() {
            let kind = magic_type.as_deref().ok_or_else(|| {
                file_err(
                    IssueCode::FileTypeUnknown {
                        kind: FileTypeKind::MagicType,
                    },
                    value,
                )
            })?;
            if !self.allowed_magic_types.iter().any(|a| a == kind) {
                return Err(not_allowed(
                    FileTypeKind::MagicType,
                    kind,
                    &self.allowed_magic_types,
                    value,
                ));
            }
        }
        if let Some(kind) = magic_type.as_deref() {
            if self.denied_magic_types.iter().any(|d| d == kind) {
                return Err(denied(FileTypeKind::MagicType, kind, value));
            }
        }

        #[cfg(feature = "file-advanced")]
        if let (true, Some(bytes)) = (self.needs_content_checks(), loaded_bytes.as_deref()) {
            if let Some(expected) = &self.sha256_hex {
                if &digest_to_hex(Sha256::digest(bytes)) != expected {
                    return Err(checksum_mismatch("SHA-256", value));
                }
            }
            if let Some(expected) = &self.md5_hex {
                if &digest_to_hex(Md5::digest(bytes)) != expected {
                    return Err(checksum_mismatch("MD5", value));
                }
            }
            if self.needs_dimensions() {
                self.check_dimensions(bytes, value)?;
            }
            if self.require_exif {
                exif::Reader::new()
                    .read_from_container(&mut std::io::Cursor::new(bytes))
                    .map_err(|_| file_err(IssueCode::MissingExif, value))?;
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

#[cfg(feature = "openapi")]
impl crate::openapi::JsonSchema for ZFile {
    fn json_schema(&self) -> serde_json::Value {
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

#[cfg(feature = "openapi")]
impl crate::openapi::CollectNestedSchemas for ZFile {}
