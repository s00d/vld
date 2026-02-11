//! # vld-config — Validate configuration files with `vld`
//!
//! Load configuration from TOML, YAML, JSON, or environment variables and
//! validate it against `vld` schemas at load time. Supports both
//! [config-rs](https://docs.rs/config) and [figment](https://docs.rs/figment)
//! as backends.
//!
//! # Quick Start (config-rs)
//!
//! ```rust,no_run
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
//!     .add_source(config::Environment::with_prefix("APP"))
//!     .build()
//!     .unwrap();
//!
//! let settings: AppSettings = from_config(&config).unwrap();
//! println!("Listening on {}:{}", settings.host, settings.port);
//! ```
//!
//! # Quick Start (figment)
//!
//! ```rust,ignore
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
//! ```

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

impl std::error::Error for VldConfigError {}

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
    T::vld_parse_value(&value).map_err(VldConfigError::Validation)
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

/// Validate a raw `serde_json::Value` as if it came from a config source.
///
/// Useful when you've already loaded the config data into a JSON value
/// (e.g. from a custom source) and just want `vld` validation.
pub fn from_value<T: VldParse>(value: &serde_json::Value) -> Result<T, VldConfigError> {
    T::vld_parse_value(value).map_err(VldConfigError::Validation)
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
    T::vld_parse_value(&value).map_err(VldConfigError::Validation)
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
