//! # vld-config — validate configuration with `vld`
//!
//! Load configuration with [config-rs](https://docs.rs/config) (feature
//! `config-rs`, default; config `>=0.14, <0.16`) or
//! [figment](https://docs.rs/figment) (feature `figment`, 0.10) and parse it
//! into a `vld` type at startup: a typed settings struct, or a
//! [`VldConfigError`] listing every invalid key. The backend crates are not
//! re-exported; depend on them yourself.
//!
//! # Quick Start (config-rs)
//!
//! ```rust,no_run
//! # #[cfg(feature = "config-rs")] {
//! use vld::prelude::*;
//! use vld_config::from_config;
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct AppSettings {
//!         pub host: String => vld::string().min(1),
//!         pub port: i64    => vld::number().int().min(1).max(65535),
//!     }
//! }
//!
//! let config = config::Config::builder()
//!     .add_source(config::File::with_name("config"))
//!     // Env values are strings; `try_parsing(true)` turns APP_PORT=8080 into a
//!     // number so it passes `number().int()`.
//!     .add_source(config::Environment::with_prefix("APP").try_parsing(true))
//!     .build()
//!     .unwrap();
//!
//! let settings: AppSettings = from_config(&config).unwrap();
//! println!("Listening on {}:{}", settings.host, settings.port);
//! # }
//! ```
//!
//! # Quick Start (figment)
//!
//! ```rust,no_run
//! # #[cfg(feature = "figment")] {
//! use vld::prelude::*;
//! use vld_config::from_figment;
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct AppSettings {
//!         pub host: String => vld::string().min(1),
//!         pub port: i64    => vld::number().int().min(1).max(65535),
//!     }
//! }
//!
//! let figment = figment::Figment::new()
//!     .merge(figment::providers::Serialized::defaults(
//!         serde_json::json!({"host": "0.0.0.0", "port": 3000}),
//!     ))
//!     .merge(figment::providers::Env::prefixed("APP_"));
//!
//! let settings: AppSettings = from_figment(&figment).unwrap();
//! # }
//! ```
//!
//! # Typed env values
//!
//! `from_config`, `from_builder` and `from_figment` validate with
//! [`vld::schema::parse_coerced`]: env values that the backend parsed into
//! numbers / booleans are turned back into text where the schema expects a
//! string (`APP_HOST=12345` into `vld::string()` is `"12345"`), and a single
//! value is wrapped into an array where the schema expects one. The text is
//! the backend's parsed value, so a lossy parse (`"01234"` → `1234`) yields
//! `"1234"`. [`from_value`] validates the JSON value **as is** (strict);
//! [`from_value_coerced`] applies the same coercion as the backends.
//!
//! Guide: <https://s00d.github.io/vld/integrations/config-cli/config>

use std::fmt;
use vld::schema::VldParse;

/// Error type for config validation.
#[derive(Debug)]
pub enum VldConfigError {
    /// Failed to extract/deserialize config into JSON.
    Source(String),
    /// Validation failed — contains all vld validation issues.
    Validation(vld::error::VldError),
}

impl fmt::Display for VldConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VldConfigError::Source(msg) => write!(f, "config error: {msg}"),
            VldConfigError::Validation(err) => write!(f, "validation error: {err}"),
        }
    }
}

impl std::error::Error for VldConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VldConfigError::Validation(err) => Some(err),
            VldConfigError::Source(_) => None,
        }
    }
}

impl From<vld::error::VldError> for VldConfigError {
    fn from(err: vld::error::VldError) -> Self {
        VldConfigError::Validation(err)
    }
}

// ---------------------------------------------------------------------------
// config-rs backend
// ---------------------------------------------------------------------------

/// Load and validate configuration from a [`config::Config`] instance.
///
/// The config is first deserialized into a `serde_json::Value`, then
/// validated and parsed using the `vld` schema via [`VldParse`].
///
/// # Errors
///
/// Returns [`VldConfigError::Source`] if the config cannot be deserialized
/// into JSON, or [`VldConfigError::Validation`] if validation fails.
///
/// # Example
///
/// ```rust,no_run
/// use vld::prelude::*;
///
/// vld::schema! {
///     #[derive(Debug)]
///     pub struct Settings {
///         pub host: String => vld::string().min(1),
///         pub port: i64    => vld::number().int().min(1).max(65535),
///     }
/// }
///
/// let config = config::Config::builder()
///     .add_source(config::File::with_name("config"))
///     .build()
///     .unwrap();
///
/// let settings: Settings = vld_config::from_config(&config).unwrap();
/// ```
#[cfg(feature = "config-rs")]
pub fn from_config<T: VldParse>(config: &config::Config) -> Result<T, VldConfigError> {
    let value: serde_json::Value = config
        .clone()
        .try_deserialize()
        .map_err(|e| VldConfigError::Source(e.to_string()))?;
    vld::schema::parse_coerced::<T>(&value).map_err(VldConfigError::Validation)
}

/// Load and validate configuration from a [`config::ConfigBuilder`].
///
/// Convenience wrapper that builds the config and validates in one step.
///
/// # Example
///
/// ```rust,no_run
/// use vld::prelude::*;
///
/// vld::schema! {
///     #[derive(Debug)]
///     pub struct Settings {
///         pub host: String => vld::string().min(1),
///         pub port: i64    => vld::number().int().min(1).max(65535),
///     }
/// }
///
/// let settings: Settings = vld_config::from_builder(
///     config::Config::builder()
///         .add_source(config::File::with_name("config"))
/// ).unwrap();
/// ```
#[cfg(feature = "config-rs")]
pub fn from_builder<T: VldParse>(
    builder: config::ConfigBuilder<config::builder::DefaultState>,
) -> Result<T, VldConfigError> {
    let config = builder
        .build()
        .map_err(|e| VldConfigError::Source(e.to_string()))?;
    from_config(&config)
}

/// Validate a raw `serde_json::Value` **strictly** (no coercion): a number
/// where the schema expects a string is an error.
///
/// Useful when you've already loaded the config data into a JSON value
/// (e.g. from a custom source) and just want `vld` validation. For values that
/// come from a typed source like env vars, use [`from_value_coerced`], which
/// behaves like `from_config` / `from_figment`.
pub fn from_value<T: VldParse>(value: &serde_json::Value) -> Result<T, VldConfigError> {
    T::vld_parse_value(value).map_err(VldConfigError::Validation)
}

/// Validate a raw `serde_json::Value` with the coercion `from_config` /
/// `from_figment` apply ([`vld::schema::parse_coerced`]): numbers / booleans
/// become text where the schema expects a string, and a single value is
/// wrapped where it expects an array.
///
/// ```
/// vld::schema! {
///     #[derive(Debug)]
///     pub struct S { pub host: String => vld::string() }
/// }
/// let v = serde_json::json!({"host": 12345});
/// assert!(vld_config::from_value::<S>(&v).is_err());
/// assert_eq!(vld_config::from_value_coerced::<S>(&v).unwrap().host, "12345");
/// ```
pub fn from_value_coerced<T: VldParse>(value: &serde_json::Value) -> Result<T, VldConfigError> {
    vld::schema::parse_coerced::<T>(value).map_err(VldConfigError::Validation)
}

// ---------------------------------------------------------------------------
// figment backend
// ---------------------------------------------------------------------------

/// Load and validate configuration from a [`figment::Figment`] instance.
///
/// The figment data is extracted into a `serde_json::Value`, then
/// validated and parsed using the `vld` schema.
///
/// # Errors
///
/// Returns [`VldConfigError::Source`] if figment extraction fails, or
/// [`VldConfigError::Validation`] if validation fails.
///
/// # Example
///
/// ```rust,no_run
/// use vld::prelude::*;
///
/// vld::schema! {
///     #[derive(Debug)]
///     pub struct Settings {
///         pub host: String => vld::string().min(1),
///         pub port: i64    => vld::number().int().min(1).max(65535),
///     }
/// }
///
/// let figment = figment::Figment::new()
///     .merge(figment::providers::Serialized::defaults(
///         serde_json::json!({"host": "localhost", "port": 8080}),
///     ));
///
/// let settings: Settings = vld_config::from_figment(&figment).unwrap();
/// ```
#[cfg(feature = "figment")]
pub fn from_figment<T: VldParse>(figment: &figment::Figment) -> Result<T, VldConfigError> {
    let value: serde_json::Value = figment
        .extract()
        .map_err(|e| VldConfigError::Source(e.to_string()))?;
    vld::schema::parse_coerced::<T>(&value).map_err(VldConfigError::Validation)
}

/// Prelude — re-exports everything you need.
pub mod prelude {
    #[cfg(feature = "figment")]
    pub use crate::from_figment;
    pub use crate::from_value;
    pub use crate::VldConfigError;
    #[cfg(feature = "config-rs")]
    pub use crate::{from_builder, from_config};
    pub use vld::prelude::*;
}
