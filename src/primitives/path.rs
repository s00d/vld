use serde_json::Value;
use std::path::PathBuf;

use crate::error::{value_type_name, IssueCode, VldError};
use crate::schema::VldSchema;

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

    pub fn within(mut self, base: impl Into<PathBuf>) -> Self {
        self.within_base = Some(base.into());
        self
    }

    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "string",
            "format": "path"
        })
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
            let msg = self.custom_type_error.clone().unwrap_or_else(|| {
                format!("Expected path string, received {}", value_type_name(value))
            });
            VldError::single_with_value(
                IssueCode::InvalidType {
                    expected: "string (path)".to_string(),
                    received: value_type_name(value),
                },
                msg,
                value,
            )
        })?;
        let p = PathBuf::from(s);

        if self.must_be_absolute && !p.is_absolute() {
            return Err(VldError::single_with_value(
                IssueCode::Custom {
                    code: "path_not_absolute".to_string(),
                },
                "Path must be absolute",
                value,
            ));
        }
        if self.must_be_relative && p.is_absolute() {
            return Err(VldError::single_with_value(
                IssueCode::Custom {
                    code: "path_not_relative".to_string(),
                },
                "Path must be relative",
                value,
            ));
        }
        if self.must_exist && !p.exists() {
            return Err(VldError::single_with_value(
                IssueCode::IoError,
                format!("Path does not exist: {}", p.display()),
                value,
            ));
        }
        if self.must_be_file && !p.is_file() {
            return Err(VldError::single_with_value(
                IssueCode::IoError,
                format!("Path is not a file: {}", p.display()),
                value,
            ));
        }
        if self.must_be_dir && !p.is_dir() {
            return Err(VldError::single_with_value(
                IssueCode::IoError,
                format!("Path is not a directory: {}", p.display()),
                value,
            ));
        }
        if let Some(base) = &self.within_base {
            let joined = if p.is_absolute() {
                p.clone()
            } else {
                base.join(&p)
            };
            let joined_canon = joined.canonicalize().unwrap_or(joined);
            let base_canon = base.canonicalize().unwrap_or_else(|_| base.clone());
            if !joined_canon.starts_with(&base_canon) {
                return Err(VldError::single_with_value(
                    IssueCode::Custom {
                        code: "path_outside_base".to_string(),
                    },
                    format!("Path must be within base directory: {}", base.display()),
                    value,
                ));
            }
        }
        Ok(p)
    }
}
