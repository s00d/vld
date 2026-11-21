//! Shared by the WebSocket modules (`tungstenite`, `axum::ws`): a text or
//! binary frame's payload is a JSON message, validated like a JSON body.

use vld::error::{IssueCode, VldError};
use vld::schema::VldParse;

/// Parse a frame's payload as JSON and validate it; malformed JSON is a
/// single `parse_error` issue at the root (as for HTTP JSON bodies).
pub(crate) fn decode<T: VldParse>(payload: &[u8]) -> Result<T, VldError> {
    let value = crate::common::parse_json_body(payload).map_err(|e| e.into_error())?;
    T::vld_parse_value(&value)
}

/// Serialize `value`, check the JSON against `T`'s own schema, return the
/// JSON text.
pub(crate) fn encode<T: serde::Serialize + VldParse>(value: &T) -> Result<String, VldError> {
    let json = serde_json::to_value(value).map_err(|e| {
        VldError::single(IssueCode::ParseError, format!("Serialization error: {e}"))
    })?;
    T::vld_parse_value(&json)?;
    Ok(json.to_string())
}

/// The HTTP 422 body (`{"error": "Validation failed", "issues": [...]}`) as text.
pub(crate) fn error_text(error: &VldError) -> String {
    crate::common::format_vld_error(error).to_string()
}
