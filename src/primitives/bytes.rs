use serde_json::Value;

use crate::error::{value_type_name, IssueCode, VldError};
use crate::schema::VldSchema;

fn b64_value(b: u8) -> Option<u8> {
    match b {
        b'A'..=b'Z' => Some(b - b'A'),
        b'a'..=b'z' => Some(26 + (b - b'a')),
        b'0'..=b'9' => Some(52 + (b - b'0')),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn decode_base64(s: &str) -> Option<Vec<u8>> {
    let bytes = s.as_bytes();
    if bytes.is_empty() || bytes.len() % 4 != 0 {
        return None;
    }

    let mut out = Vec::with_capacity((bytes.len() / 4) * 3);
    let mut i = 0;
    while i < bytes.len() {
        let c0 = bytes[i];
        let c1 = bytes[i + 1];
        let c2 = bytes[i + 2];
        let c3 = bytes[i + 3];
        i += 4;

        let v0 = b64_value(c0)?;
        let v1 = b64_value(c1)?;

        if c2 == b'=' {
            if c3 != b'=' || i != bytes.len() {
                return None;
            }
            out.push((v0 << 2) | (v1 >> 4));
            break;
        }

        let v2 = b64_value(c2)?;
        out.push((v0 << 2) | (v1 >> 4));
        out.push((v1 << 4) | (v2 >> 2));

        if c3 == b'=' {
            if i != bytes.len() {
                return None;
            }
            break;
        }

        let v3 = b64_value(c3)?;
        out.push((v2 << 6) | v3);
    }

    Some(out)
}

#[derive(Clone)]
pub struct ZBytes {
    min_len: Option<usize>,
    max_len: Option<usize>,
    exact_len: Option<usize>,
    base64_mode: bool,
    custom_type_error: Option<String>,
}

impl ZBytes {
    pub fn new() -> Self {
        Self {
            min_len: None,
            max_len: None,
            exact_len: None,
            base64_mode: false,
            custom_type_error: None,
        }
    }

    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    pub fn min_len(mut self, len: usize) -> Self {
        self.min_len = Some(len);
        self
    }

    pub fn max_len(mut self, len: usize) -> Self {
        self.max_len = Some(len);
        self
    }

    pub fn len(mut self, len: usize) -> Self {
        self.exact_len = Some(len);
        self
    }

    pub fn non_empty(self) -> Self {
        self.min_len(1)
    }

    pub fn base64(mut self) -> Self {
        self.base64_mode = true;
        self
    }

    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        if self.base64_mode {
            let mut schema = serde_json::json!({
                "type": "string",
                "format": "byte"
            });
            if let Some(min) = self.min_len {
                schema["minLength"] = serde_json::json!(min);
            }
            if let Some(max) = self.max_len {
                schema["maxLength"] = serde_json::json!(max);
            }
            if let Some(exact) = self.exact_len {
                schema["minLength"] = serde_json::json!(exact);
                schema["maxLength"] = serde_json::json!(exact);
            }
            schema
        } else {
            let mut schema = serde_json::json!({
                "type": "array",
                "items": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 255
                }
            });
            if let Some(min) = self.min_len {
                schema["minItems"] = serde_json::json!(min);
            }
            if let Some(max) = self.max_len {
                schema["maxItems"] = serde_json::json!(max);
            }
            if let Some(exact) = self.exact_len {
                schema["minItems"] = serde_json::json!(exact);
                schema["maxItems"] = serde_json::json!(exact);
            }
            schema
        }
    }
}

impl Default for ZBytes {
    fn default() -> Self {
        Self::new()
    }
}

impl VldSchema for ZBytes {
    type Output = Vec<u8>;

    fn parse_value(&self, value: &Value) -> Result<Vec<u8>, VldError> {
        let type_err = || {
            let expected = if self.base64_mode {
                "bytes array or base64 string"
            } else {
                "bytes array"
            };
            let msg = self.custom_type_error.clone().unwrap_or_else(|| {
                format!("Expected {}, received {}", expected, value_type_name(value))
            });
            VldError::single_with_value(
                IssueCode::InvalidType {
                    expected: expected.to_string(),
                    received: value_type_name(value),
                },
                msg,
                value,
            )
        };

        let bytes = match value {
            Value::Array(arr) => {
                let mut out = Vec::with_capacity(arr.len());
                for item in arr {
                    let n = item.as_u64().ok_or_else(type_err)?;
                    if n > 255 {
                        return Err(VldError::single_with_value(
                            IssueCode::TooBig {
                                maximum: 255.0,
                                inclusive: true,
                            },
                            "Byte value must be in range 0..=255",
                            item,
                        ));
                    }
                    out.push(n as u8);
                }
                out
            }
            Value::String(s) if self.base64_mode => decode_base64(s).ok_or_else(|| {
                VldError::single_with_value(
                    IssueCode::Custom {
                        code: "invalid_base64".to_string(),
                    },
                    "Invalid Base64 byte string",
                    value,
                )
            })?,
            _ => return Err(type_err()),
        };

        let mut errors = VldError::new();
        if let Some(min) = self.min_len {
            if bytes.len() < min {
                errors.push_with_value(
                    IssueCode::TooSmall {
                        minimum: min as f64,
                        inclusive: true,
                    },
                    format!("Bytes length must be at least {}", min),
                    value,
                );
            }
        }
        if let Some(max) = self.max_len {
            if bytes.len() > max {
                errors.push_with_value(
                    IssueCode::TooBig {
                        maximum: max as f64,
                        inclusive: true,
                    },
                    format!("Bytes length must be at most {}", max),
                    value,
                );
            }
        }
        if let Some(exact) = self.exact_len {
            if bytes.len() != exact {
                errors.push_with_value(
                    IssueCode::Custom {
                        code: "invalid_length".to_string(),
                    },
                    format!("Bytes length must be exactly {}", exact),
                    value,
                );
            }
        }

        if errors.is_empty() {
            Ok(bytes)
        } else {
            Err(errors)
        }
    }
}
