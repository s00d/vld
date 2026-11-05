use serde_json::Value;
use std::path::{Component, Path, PathBuf};

use crate::error::{IssueCode, VldError};
use crate::schema::VldSchema;

/// Resolve `.` and `..` without touching the filesystem. `..` never climbs
/// above the root (or above the start of a relative path).
fn lexical_normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !matches!(
                    out.components().next_back(),
                    None | Some(Component::RootDir | Component::Prefix(_))
                ) {
                    out.pop();
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// Best-effort real path: canonicalize the deepest existing ancestor (so
/// symlinks there are followed) and append the remaining, lexically
/// normalized components.
fn real_path(p: &Path) -> PathBuf {
    if let Ok(c) = p.canonicalize() {
        return c;
    }
    let norm = lexical_normalize(p);
    let mut existing = norm.as_path();
    let mut rest = vec![];
    loop {
        if let Ok(c) = existing.canonicalize() {
            return rest
                .iter()
                .rev()
                .fold(c, |acc: PathBuf, part| acc.join(part));
        }
        match (existing.file_name(), existing.parent()) {
            (Some(name), Some(parent)) => {
                rest.push(name.to_os_string());
                existing = parent;
            }
            _ => return norm,
        }
    }
}

/// Join `p` onto `base` (absolute `p` is kept as is) and check that the result
/// stays inside `base` after resolving `..`, `.` and existing symlinks.
///
/// Returns the joined path (`base.join(p)`, not canonicalized) on success.
pub(crate) fn resolve_within(base: &Path, p: &Path) -> Option<PathBuf> {
    let joined = if p.is_absolute() {
        p.to_path_buf()
    } else {
        base.join(p)
    };
    real_path(&joined)
        .starts_with(real_path(base))
        .then_some(joined)
}

pub(crate) fn outside_base_error(base: &Path, value: &Value) -> VldError {
    VldError::from_code_with_value(
        IssueCode::PathOutsideBase {
            base: base.display().to_string(),
        },
        value,
    )
}

/// Schema for filesystem paths. Created via [`vld::path()`](crate::path)
/// (read its security note: confine untrusted paths with [`within`](Self::within)).
#[derive(Clone)]
pub struct ZPath {
    must_exist: bool,
    must_be_file: bool,
    must_be_dir: bool,
    must_be_absolute: bool,
    must_be_relative: bool,
    within_base: Option<PathBuf>,
    custom_type_error: Option<String>,
}

impl ZPath {
    pub fn new() -> Self {
        Self {
            must_exist: false,
            must_be_file: false,
            must_be_dir: false,
            must_be_absolute: false,
            must_be_relative: false,
            within_base: None,
            custom_type_error: None,
        }
    }

    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    pub fn exists(mut self) -> Self {
        self.must_exist = true;
        self
    }

    pub fn file(mut self) -> Self {
        self.must_be_file = true;
        self.must_exist = true;
        self
    }

    pub fn dir(mut self) -> Self {
        self.must_be_dir = true;
        self.must_exist = true;
        self
    }

    pub fn absolute(mut self) -> Self {
        self.must_be_absolute = true;
        self
    }

    pub fn relative(mut self) -> Self {
        self.must_be_relative = true;
        self
    }

    /// Require the path to stay inside `base`.
    ///
    /// Relative inputs are resolved against `base` (not the current directory)
    /// and the **joined** path `base.join(input)` is returned, so later file
    /// operations hit the checked location. `..`/`.` are resolved and existing
    /// symlinks followed before the containment check, so `"../etc/x"` or
    /// `"/base/../etc/x"` are rejected even when they do not exist.
    pub fn within(mut self, base: impl Into<PathBuf>) -> Self {
        self.within_base = Some(base.into());
        self
    }
}

impl Default for ZPath {
    fn default() -> Self {
        Self::new()
    }
}

impl VldSchema for ZPath {
    type Output = PathBuf;

    fn parse_value(&self, value: &Value) -> Result<PathBuf, VldError> {
        let s = value.as_str().ok_or_else(|| {
            crate::primitives::type_error(&self.custom_type_error, "path", value)
        })?;
        let mut p = PathBuf::from(s);

        if self.must_be_absolute && !p.is_absolute() {
            return Err(VldError::from_code_with_value(
                IssueCode::PathNotAbsolute,
                value,
            ));
        }
        if self.must_be_relative && p.is_absolute() {
            return Err(VldError::from_code_with_value(
                IssueCode::PathNotRelative,
                value,
            ));
        }
        if let Some(base) = &self.within_base {
            p = resolve_within(base, &p).ok_or_else(|| outside_base_error(base, value))?;
        }
        let fail = |code| Err(VldError::from_code_with_value(code, value));
        if self.must_exist && !p.exists() {
            return fail(IssueCode::PathNotFound);
        }
        if self.must_be_file && !p.is_file() {
            return fail(IssueCode::PathNotFile);
        }
        if self.must_be_dir && !p.is_dir() {
            return fail(IssueCode::PathNotDir);
        }
        Ok(p)
    }
}

#[cfg(feature = "openapi")]
impl crate::openapi::JsonSchema for ZPath {
    fn json_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "string",
            "format": "path"
        })
    }
}

#[cfg(feature = "openapi")]
impl crate::openapi::CollectNestedSchemas for ZPath {}
