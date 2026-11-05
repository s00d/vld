use serde_json::Value;

use crate::error::{FormatKind, IssueCode, PathSegment, SizeOrigin, VldError};
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

pub(crate) fn decode_base64(s: &str) -> Option<Vec<u8>> {
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

fn decode_base64_url(s: &str) -> Option<Vec<u8>> {
    let mut std = s.replace('-', "+").replace('_', "/");
    let rem = std.len() % 4;
    if rem != 0 {
        std.push_str(&"=".repeat(4 - rem));
    }
    decode_base64(&std)
}

fn decode_hex(s: &str) -> Option<Vec<u8>> {
    let raw = s.strip_prefix("0x").unwrap_or(s);
    if raw.is_empty() || raw.len() % 2 != 0 || !raw.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = Vec::with_capacity(raw.len() / 2);
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = bytes[i] as char;
        let lo = bytes[i + 1] as char;
        let h = hi.to_digit(16)? as u8;
        let l = lo.to_digit(16)? as u8;
        out.push((h << 4) | l);
        i += 2;
    }
    Some(out)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BytesStringMode {
    Off,
    Base64,
    Base64Url,
    Hex,
}

#[derive(Clone)]
pub struct ZBytes {
    min_len: Option<usize>,
    max_len: Option<usize>,
    exact_len: Option<usize>,
    string_mode: BytesStringMode,
    custom_type_error: Option<String>,
}

impl ZBytes {
    pub fn new() -> Self {
        Self {
            min_len: None,
            max_len: None,
            exact_len: None,
            string_mode: BytesStringMode::Off,
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
        self.string_mode = BytesStringMode::Base64;
        self
    }

    pub fn base64url(mut self) -> Self {
        self.string_mode = BytesStringMode::Base64Url;
        self
    }

    pub fn hex(mut self) -> Self {
        self.string_mode = BytesStringMode::Hex;
        self
    }

    /// JSON Schema. In string modes `minLength`/`maxLength` describe the
    /// *encoded* string length (Base64: `4*ceil(n/3)` padded; Base64URL may be
    /// unpadded; hex: `2n`, plus an optional `0x` prefix).
    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        let mins = [self.min_len, self.exact_len].into_iter().flatten();
        let maxs = [self.max_len, self.exact_len].into_iter().flatten();
        let padded = |n: usize| n.div_ceil(3) * 4;
        let unpadded = |n: usize| (n * 4).div_ceil(3);
        type Enc = fn(usize) -> usize;
        let (format, min_enc, max_enc): (&str, Enc, Enc) = match self.string_mode {
            BytesStringMode::Off => {
                let mut schema = serde_json::json!({
                    "type": "array",
                    "items": { "type": "integer", "minimum": 0, "maximum": 255 }
                });
                super::length_schema(&mut schema, ("minItems", "maxItems"), mins, maxs);
                return schema;
            }
            BytesStringMode::Base64 => ("byte", padded, padded),
            BytesStringMode::Base64Url => ("byte-base64url", unpadded, padded),
            BytesStringMode::Hex => ("byte-hex", |n| n * 2, |n| n * 2 + 2),
        };
        let mut schema = serde_json::json!({ "type": "string", "format": format });
        super::length_schema(
            &mut schema,
            ("minLength", "maxLength"),
            mins.map(min_enc),
            maxs.map(max_enc),
        );
        schema
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
            let expected = if self.string_mode != BytesStringMode::Off {
                "bytes array or encoded string"
            } else {
                "bytes array"
            };
            crate::primitives::type_error(&self.custom_type_error, expected, value)
        };

        let bytes = match value {
            Value::Array(arr) => {
                let mut out = Vec::with_capacity(arr.len());
                for (i, item) in arr.iter().enumerate() {
                    let n = item.as_u64().ok_or_else(|| {
                        crate::primitives::type_error(
                            &self.custom_type_error,
                            "byte (integer 0..=255)",
                            item,
                        )
                        .with_prefix(PathSegment::Index(i))
                    })?;
                    if n > 255 {
                        return Err(VldError::from_code_with_value(
                            IssueCode::TooBig {
                                origin: SizeOrigin::Number,
                                maximum: 255.into(),
                                inclusive: true,
                            },
                            item,
                        )
                        .with_prefix(PathSegment::Index(i)));
                    }
                    out.push(n as u8);
                }
                out
            }
            Value::String(s) if self.string_mode != BytesStringMode::Off => {
                match self.string_mode {
                    BytesStringMode::Base64 => decode_base64(s).ok_or_else(|| {
                        VldError::from_code_with_value(
                            IssueCode::InvalidFormat {
                                format: FormatKind::Base64,
                            },
                            value,
                        )
                    })?,
                    BytesStringMode::Base64Url => decode_base64_url(s).ok_or_else(|| {
                        VldError::from_code_with_value(
                            IssueCode::InvalidFormat {
                                format: FormatKind::Base64Url,
                            },
                            value,
                        )
                    })?,
                    BytesStringMode::Hex => decode_hex(s).ok_or_else(|| {
                        VldError::from_code_with_value(
                            IssueCode::InvalidFormat {
                                format: FormatKind::Hex,
                            },
                            value,
                        )
                    })?,
                    BytesStringMode::Off => return Err(type_err()),
                }
            }
            _ => return Err(type_err()),
        };

        let mut errors = VldError::new();
        if let Some(min) = self.min_len {
            if bytes.len() < min {
                errors.push_code_with_value(
                    IssueCode::TooSmall {
                        origin: SizeOrigin::Bytes,
                        minimum: min.into(),
                        inclusive: true,
                    },
                    value,
                );
            }
        }
        if let Some(max) = self.max_len {
            if bytes.len() > max {
                errors.push_code_with_value(
                    IssueCode::TooBig {
                        origin: SizeOrigin::Bytes,
                        maximum: max.into(),
                        inclusive: true,
                    },
                    value,
                );
            }
        }
        if let Some(exact) = self.exact_len {
            if bytes.len() != exact {
                errors.push_code_with_value(
                    IssueCode::InvalidLength {
                        origin: SizeOrigin::Bytes,
                        expected: exact,
                    },
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
