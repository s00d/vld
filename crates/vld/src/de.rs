//! Bridge to serde: validate a field while serde deserializes the struct, so
//! one decoder does both.
//!
//! - [`parse`] — `#[serde(deserialize_with = "vld::de::parse")]` on a field
//!   whose type implements [`VldParse`] (`schema!` / `#[derive(Validate)]`).
//! - [`deserialize_fn!`](crate::deserialize_fn) — a `deserialize_with`
//!   function from any schema expression.
//! - [`with`] — the building block of both: parse with a schema you hold.
//!
//! ```
//! use serde::Deserialize;
//!
//! vld::deserialize_fn!(fn port -> u16 = vld::u16().min(1));
//! vld::deserialize_fn!(fn volume -> i64 = vld::number().int().min(0).max(100).clamp());
//!
//! #[derive(Deserialize)]
//! struct Settings {
//!     #[serde(deserialize_with = "port")]
//!     port: u16,
//!     #[serde(deserialize_with = "volume")]
//!     volume: i64,
//! }
//!
//! let s: Settings = serde_json::from_str(r#"{"port": 8080, "volume": 250}"#).unwrap();
//! assert_eq!((s.port, s.volume), (8080, 100));
//! let err = serde_json::from_str::<Settings>(r#"{"port": 0, "volume": 1}"#).err().unwrap();
//! assert!(err.to_string().contains("at least 1"), "{err}");
//! ```
//!
//! A `deserialize_with` field is required by serde; add `#[serde(default)]`
//! to allow a missing key. The vld error (all issues, as text) becomes the
//! serde error; for structured issues parse with vld itself.

use serde::de::{Deserialize, Deserializer, Error};

use crate::schema::{VldParse, VldSchema};

/// Parse the value with `schema`; a failure becomes `D::Error::custom`.
pub fn with<'de, D, S>(schema: &S, deserializer: D) -> Result<S::Output, D::Error>
where
    D: Deserializer<'de>,
    S: VldSchema,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    schema.parse_value(&value).map_err(D::Error::custom)
}

/// `deserialize_with` for a field whose type implements [`VldParse`].
pub fn parse<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: VldParse,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    T::vld_parse_value(&value).map_err(D::Error::custom)
}
