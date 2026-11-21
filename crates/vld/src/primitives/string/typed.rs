//! String schemas with a typed output: [`ZUuid`] (`uuid::Uuid`) and
//! [`ZUrl`] (`url::Url`).

use serde_json::Value;

use super::ZString;
use crate::error::{IssueCode, StringValidation, VldError};
use crate::schema::VldSchema;

/// A [`ZString`] whose (checked) value is parsed into `T`. Created via
/// [`vld::uuid()`](crate::uuid) / [`vld::url()`](crate::url).
///
/// The string checks run first and report the same issues as the
/// `ZString` they come from; the JSON Schema is the string's.
pub struct ZParsedString<T> {
    inner: ZString,
    parse: fn(&str) -> Option<T>,
    validation: StringValidation,
}

/// `vld::uuid()`: a UUID string parsed into [`uuid::Uuid`].
pub type ZUuid = ZParsedString<uuid::Uuid>;
/// `vld::url()`: an absolute URL parsed into [`url::Url`].
pub type ZUrl = ZParsedString<url::Url>;

impl<T> Clone for ZParsedString<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            parse: self.parse,
            validation: self.validation.clone(),
        }
    }
}

impl<T> ZParsedString<T> {
    /// Add string checks or transforms (the closure gets the current
    /// string schema, its format check included).
    ///
    /// ```
    /// use vld::prelude::*;
    /// let id = vld::uuid().checks(|s| s.uuid_v7().to_lowercase());
    /// let v = id.parse(r#""01890F57-5A7B-7F8B-BFD3-63F8E7C6F4B8""#).unwrap();
    /// assert_eq!(v.get_version_num(), 7);
    /// assert!(id.parse(r#""550e8400-e29b-41d4-a716-446655440000""#).is_err());
    /// ```
    pub fn checks(mut self, f: impl FnOnce(ZString) -> ZString) -> Self {
        self.inner = f(self.inner);
        self
    }
}

impl ZUuid {
    /// Create the schema (the same as [`vld::uuid()`](crate::uuid)).
    pub fn new() -> Self {
        Self {
            inner: ZString::new().uuid(),
            parse: |s| uuid::Uuid::parse_str(s).ok(),
            validation: StringValidation::Uuid,
        }
    }
}

impl Default for ZUuid {
    fn default() -> Self {
        Self::new()
    }
}

impl ZUrl {
    /// Create the schema (the same as [`vld::url()`](crate::url)).
    pub fn new() -> Self {
        Self {
            inner: ZString::new().uri(),
            parse: |s| url::Url::parse(s).ok(),
            validation: StringValidation::Uri,
        }
    }
}

impl Default for ZUrl {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> VldSchema for ZParsedString<T> {
    type Output = T;

    fn parse_value(&self, value: &Value) -> Result<T, VldError> {
        let s = self.inner.parse_value(value)?;
        // The format check already accepted `s`; a custom `checks` chain
        // could still have transformed it into something unparsable.
        (self.parse)(&s).ok_or_else(|| {
            VldError::from_code_with_value(
                IssueCode::InvalidString {
                    validation: self.validation.clone(),
                },
                &Value::String(s),
            )
        })
    }
}

#[cfg(feature = "openapi")]
impl<T> crate::openapi::JsonSchema for ZParsedString<T> {
    fn json_schema(&self) -> Value {
        self.inner.json_schema()
    }
}

#[cfg(feature = "openapi")]
impl<T> crate::openapi::CollectNestedSchemas for ZParsedString<T> {}
