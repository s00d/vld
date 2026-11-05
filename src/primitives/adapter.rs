//! Adapters for existing Rust types: [`ZFromSerde`], [`ZFromStr`], [`ZEnumOf`].

use serde_json::Value;
use std::fmt::Display;
use std::marker::PhantomData;
use std::str::FromStr;

use crate::error::{FormatKind, IssueCode, VldError};
use crate::schema::VldSchema;

/// Schema that deserializes the JSON value into any `T: DeserializeOwned`.
/// Created via [`vld::from_serde()`](crate::from_serde).
///
/// A serde error becomes one `parse_error` issue whose message is the serde
/// message. JSON Schema is `{}` (any) unless set with [`schema`](Self::schema).
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// #[derive(Debug, PartialEq, serde::Deserialize)]
/// #[serde(rename_all = "lowercase")]
/// enum Role { Admin, User }
///
/// let role = vld::from_serde::<Role>();
/// assert_eq!(role.parse(r#""admin""#).unwrap(), Role::Admin);
/// let err = role.parse(r#""root""#).unwrap_err();
/// assert!(err.issues[0].message.contains("unknown variant"));
/// ```
pub struct ZFromSerde<T> {
    schema: Value,
    _type: PhantomData<fn() -> T>,
}

impl<T> ZFromSerde<T> {
    pub fn new() -> Self {
        Self {
            schema: serde_json::json!({}),
            _type: PhantomData,
        }
    }

    /// JSON Schema to report for this type (default `{}`).
    pub fn schema(mut self, schema: Value) -> Self {
        self.schema = schema;
        self
    }
}

impl<T> Default for ZFromSerde<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for ZFromSerde<T> {
    fn clone(&self) -> Self {
        Self {
            schema: self.schema.clone(),
            _type: PhantomData,
        }
    }
}

impl<T: serde::de::DeserializeOwned> VldSchema for ZFromSerde<T> {
    type Output = T;

    fn parse_value(&self, value: &Value) -> Result<T, VldError> {
        T::deserialize(value)
            .map_err(|e| VldError::single_with_value(IssueCode::ParseError, e.to_string(), value))
    }
}

#[cfg(feature = "openapi")]
impl<T> crate::openapi::JsonSchema for ZFromSerde<T> {
    fn json_schema(&self) -> Value {
        self.schema.clone()
    }
}

#[cfg(feature = "openapi")]
impl<T> crate::openapi::CollectNestedSchemas for ZFromSerde<T> {}

/// Schema that parses a JSON string with `T: FromStr`.
/// Created via [`vld::from_str()`](crate::from_str).
///
/// Non-string input fails with `invalid_type`; a parse error becomes one
/// `invalid_format` (`from_str`) issue whose message is the error's `Display`.
/// JSON Schema is `{"type": "string"}` unless set with [`schema`](Self::schema).
///
/// # Example
/// ```
/// use vld::prelude::*;
/// use std::net::IpAddr;
///
/// let ip = vld::from_str::<IpAddr>();
/// assert_eq!(ip.parse(r#""10.0.0.1""#).unwrap(), "10.0.0.1".parse::<IpAddr>().unwrap());
/// assert!(ip.parse(r#""10.0.0.256""#).is_err());
/// assert!(ip.parse("42").is_err());
/// ```
pub struct ZFromStr<T> {
    schema: Value,
    custom_type_error: Option<String>,
    _type: PhantomData<fn() -> T>,
}

impl<T> ZFromStr<T> {
    pub fn new() -> Self {
        Self {
            schema: serde_json::json!({"type": "string"}),
            custom_type_error: None,
            _type: PhantomData,
        }
    }

    /// JSON Schema to report for this type (default `{"type": "string"}`).
    pub fn schema(mut self, schema: Value) -> Self {
        self.schema = schema;
        self
    }

    /// Set a custom error message for type mismatch (non-string input).
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }
}

impl<T> Default for ZFromStr<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for ZFromStr<T> {
    fn clone(&self) -> Self {
        Self {
            schema: self.schema.clone(),
            custom_type_error: self.custom_type_error.clone(),
            _type: PhantomData,
        }
    }
}

impl<T> VldSchema for ZFromStr<T>
where
    T: FromStr,
    T::Err: Display,
{
    type Output = T;

    fn parse_value(&self, value: &Value) -> Result<T, VldError> {
        let s = value.as_str().ok_or_else(|| {
            crate::primitives::type_error(&self.custom_type_error, "string", value)
        })?;
        s.parse::<T>().map_err(|e| {
            VldError::single_with_value(
                IssueCode::InvalidFormat {
                    format: FormatKind::FromStr,
                },
                e.to_string(),
                value,
            )
        })
    }
}

#[cfg(feature = "openapi")]
impl<T> crate::openapi::JsonSchema for ZFromStr<T> {
    fn json_schema(&self) -> Value {
        self.schema.clone()
    }
}

#[cfg(feature = "openapi")]
impl<T> crate::openapi::CollectNestedSchemas for ZFromStr<T> {}

/// Schema mapping fixed strings to values of `T`.
/// Created via [`vld::enum_of()`](crate::enum_of).
///
/// Unknown strings fail with `invalid_enum_value` listing the options.
/// JSON Schema: `{"type": "string", "enum": [...]}`.
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// #[derive(Debug, Clone, PartialEq)]
/// enum Role { Admin, User }
///
/// let role = vld::enum_of(&[("admin", Role::Admin), ("user", Role::User)]);
/// assert_eq!(role.parse(r#""user""#).unwrap(), Role::User);
/// assert!(role.parse(r#""root""#).is_err());
/// ```
#[derive(Clone)]
pub struct ZEnumOf<T> {
    variants: Vec<(String, T)>,
    custom_type_error: Option<String>,
}

impl<T: Clone> ZEnumOf<T> {
    pub fn new(variants: &[(&str, T)]) -> Self {
        Self {
            variants: variants
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
            custom_type_error: None,
        }
    }

    /// Set a custom error message for type mismatch (non-string input).
    pub fn type_error(mut self, msg: impl Into<String>) -> Self {
        self.custom_type_error = Some(msg.into());
        self
    }

    fn names(&self) -> Vec<String> {
        self.variants.iter().map(|(k, _)| k.clone()).collect()
    }
}

impl<T: Clone> VldSchema for ZEnumOf<T> {
    type Output = T;

    fn parse_value(&self, value: &Value) -> Result<T, VldError> {
        let s = value.as_str().ok_or_else(|| {
            crate::primitives::type_error(&self.custom_type_error, "string", value)
        })?;
        self.variants
            .iter()
            .find(|(k, _)| k == s)
            .map(|(_, v)| v.clone())
            .ok_or_else(|| {
                VldError::from_code_with_value(
                    IssueCode::InvalidEnumValue {
                        options: self.names(),
                    },
                    value,
                )
            })
    }
}

#[cfg(feature = "openapi")]
impl<T: Clone> crate::openapi::JsonSchema for ZEnumOf<T> {
    fn json_schema(&self) -> Value {
        serde_json::json!({"type": "string", "enum": self.names()})
    }
}

#[cfg(feature = "openapi")]
impl<T> crate::openapi::CollectNestedSchemas for ZEnumOf<T> {}
