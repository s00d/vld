use serde_json::Value;

use crate::error::{IssueCode, VldError};
use crate::schema::VldSchema;

#[derive(Clone)]
pub struct ZJsonValue {
    require_object: bool,
    require_array: bool,
    required_keys: Vec<String>,
    max_depth: Option<usize>,
    custom_type_error: Option<String>,
}

impl ZJsonValue {
    pub fn new() -> Self {
        Self {
            require_object: false,
            require_array: false,
            required_keys: Vec::new(),
            max_depth: None,
            custom_type_error: None,
        }
    }

    /// Set a custom error message for type mismatch (`object()` / `array()`
    /// / `require_key` on a non-object).
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    pub fn object(mut self) -> Self {
        self.require_object = true;
        self.require_array = false;
        self
    }

    pub fn array(mut self) -> Self {
        self.require_array = true;
        self.require_object = false;
        self
    }

    pub fn require_key(mut self, key: impl Into<String>) -> Self {
        self.required_keys.push(key.into());
        self
    }

    pub fn max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        let mut schema = serde_json::json!({});
        if self.require_object {
            schema["type"] = serde_json::json!("object");
            if !self.required_keys.is_empty() {
                schema["required"] = serde_json::json!(self.required_keys);
            }
        } else if self.require_array {
            schema["type"] = serde_json::json!("array");
        }
        if let Some(depth) = self.max_depth {
            schema["x-maxDepth"] = serde_json::json!(depth);
        }
        schema
    }
}

impl Default for ZJsonValue {
    fn default() -> Self {
        Self::new()
    }
}

fn depth(value: &Value) -> usize {
    match value {
        Value::Array(arr) => 1 + arr.iter().map(depth).max().unwrap_or(0),
        Value::Object(map) => 1 + map.values().map(depth).max().unwrap_or(0),
        _ => 1,
    }
}

impl VldSchema for ZJsonValue {
    type Output = Value;

    fn parse_value(&self, value: &Value) -> Result<Self::Output, VldError> {
        if self.require_object && !value.is_object() {
            return Err(super::type_error(
                &self.custom_type_error,
                "object",
                "object JSON value",
                value,
            ));
        }
        if self.require_array && !value.is_array() {
            return Err(super::type_error(
                &self.custom_type_error,
                "array",
                "array JSON value",
                value,
            ));
        }
        if !self.required_keys.is_empty() {
            let obj = value.as_object().ok_or_else(|| {
                super::type_error(
                    &self.custom_type_error,
                    "object",
                    "object (required keys need an object)",
                    value,
                )
            })?;
            for k in &self.required_keys {
                if !obj.contains_key(k) {
                    return Err(VldError::single_with_value(
                        IssueCode::MissingField,
                        format!("Missing required key `{}`", k),
                        value,
                    ));
                }
            }
        }
        if let Some(max_depth) = self.max_depth {
            let d = depth(value);
            if d > max_depth {
                return Err(VldError::single_with_value(
                    IssueCode::Custom {
                        code: "json_depth_exceeded".to_string(),
                    },
                    format!("JSON depth {} exceeds max {}", d, max_depth),
                    value,
                ));
            }
        }
        Ok(value.clone())
    }
}
