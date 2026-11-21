//! JSON payload encoding shared by the message modules (`rumqttc`, `nats`,
//! `centrifuge`): validate then serialize, parse then validate. Each module
//! maps [`CodecError`] into its own error type.

/// What can go wrong while encoding / decoding a payload.
pub(crate) enum CodecError {
    Validation(vld::error::VldError),
    Serialization(String),
    Deserialization(String),
}

/// Serialize `value` to JSON, validate it against its own schema and return
/// the JSON bytes.
pub(crate) fn encode<V>(value: &V) -> Result<Vec<u8>, CodecError>
where
    V: serde::Serialize + vld::schema::VldParse,
{
    let json = serde_json::to_value(value).map_err(|e| CodecError::Serialization(e.to_string()))?;
    V::vld_parse_value(&json).map_err(CodecError::Validation)?;
    serde_json::to_vec(&json).map_err(|e| CodecError::Serialization(e.to_string()))
}

/// Parse a JSON payload and validate it against `T`.
pub(crate) fn decode<T>(payload: &[u8]) -> Result<T, CodecError>
where
    T: vld::schema::VldParse,
{
    let json: serde_json::Value =
        serde_json::from_slice(payload).map_err(|e| CodecError::Deserialization(e.to_string()))?;
    T::vld_parse_value(&json).map_err(CodecError::Validation)
}

/// `impl From<CodecError> for $Err` for an error enum with `Validation`,
/// `Serialization` and `Deserialization` variants.
macro_rules! __vld_codec_from {
    ($Err:ident) => {
        impl From<$crate::codec::CodecError> for $Err {
            fn from(e: $crate::codec::CodecError) -> Self {
                match e {
                    $crate::codec::CodecError::Validation(e) => $Err::Validation(e),
                    $crate::codec::CodecError::Serialization(e) => $Err::Serialization(e),
                    $crate::codec::CodecError::Deserialization(e) => $Err::Deserialization(e),
                }
            }
        }
    };
}
pub(crate) use __vld_codec_from;
