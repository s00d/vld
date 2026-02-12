//! # vld-tauri — Tauri validation for `vld`
//!
//! Provides helpers for validating [Tauri](https://tauri.app/) IPC commands,
//! events, state, plugin config and channel messages using `vld` schemas.
//!
//! **This crate does not depend on `tauri` itself** — it only needs `vld`,
//! `serde` and `serde_json`, keeping the dependency tree minimal.
//! You add `tauri` as a separate dependency in your app.
//!
//! # Overview
//!
//! | Item | Use case |
//! |------|----------|
//! | [`validate`] | General-purpose JSON validation |
//! | [`validate_args`] | Validate a raw JSON string |
//! | [`validate_event`] | Validate incoming event payloads |
//! | [`validate_state`] | Validate app state / config at init |
//! | [`validate_channel_message`] | Validate outgoing channel messages |
//! | [`validate_plugin_config`] | Validate plugin JSON config |
//! | [`VldTauriError`] | Serializable error for `#[tauri::command]` results |
//! | [`VldPayload<T>`] | Auto-validating `Deserialize` wrapper |
//! | [`VldEvent<T>`] | Auto-validating `Deserialize` wrapper for events |
//!
//! # Usage patterns
//!
//! ## IPC commands — explicit validation (recommended)
//!
//! Accept `serde_json::Value` and validate manually.
//! The frontend receives structured error JSON on failure.
//!
//! ```rust,ignore
//! use vld_tauri::prelude::*;
//!
//! vld::schema! {
//!     #[derive(Debug, Clone, serde::Serialize)]
//!     pub struct CreateUser {
//!         pub name: String  => vld::string().min(2).max(50),
//!         pub email: String => vld::string().email(),
//!     }
//! }
//!
//! #[tauri::command]
//! fn create_user(payload: serde_json::Value) -> Result<String, VldTauriError> {
//!     let user = validate::<CreateUser>(payload)?;
//!     Ok(format!("Created {}", user.name))
//! }
//! ```
//!
//! ## IPC commands — auto-validated payload
//!
//! `VldPayload<T>` implements `Deserialize` and validates during
//! deserialization. If validation fails, Tauri reports a deserialization
//! error with the full validation message.
//!
//! ```rust,ignore
//! #[tauri::command]
//! fn create_user(payload: VldPayload<CreateUser>) -> Result<String, VldTauriError> {
//!     Ok(format!("Created {}", payload.name))
//! }
//! ```
//!
//! ## Event payloads
//!
//! Validate data from `emit()`/`listen()`:
//!
//! ```rust,ignore
//! use tauri::{Emitter, Listener};
//!
//! // Backend: validate incoming event from frontend
//! app.listen("user:update", |event| {
//!     let payload: serde_json::Value = serde_json::from_str(event.payload()).unwrap();
//!     match validate_event::<UserUpdate>(payload) {
//!         Ok(update) => println!("Valid update: {:?}", update),
//!         Err(e)     => eprintln!("Bad event: {e}"),
//!     }
//! });
//!
//! // Or auto-validate with VldEvent<T>:
//! let event: VldEvent<UserUpdate> = serde_json::from_str(event.payload()).unwrap();
//! ```
//!
//! ## State validation at init
//!
//! ```rust,ignore
//! let config_json = std::fs::read_to_string("config.json").unwrap();
//! let config = validate_state::<AppConfig>(
//!     serde_json::from_str(&config_json).unwrap()
//! ).expect("Invalid app config");
//! app.manage(config);
//! ```
//!
//! ## Plugin config validation
//!
//! ```rust,ignore
//! let plugin_cfg: serde_json::Value = /* from tauri.conf.json */;
//! let cfg = validate_plugin_config::<MyPluginConfig>(plugin_cfg)
//!     .expect("Invalid plugin config");
//! ```
//!
//! ## Channel message validation
//!
//! ```rust,ignore
//! #[tauri::command]
//! fn stream(channel: Channel<serde_json::Value>) -> Result<(), VldTauriError> {
//!     let msg = ProgressUpdate { percent: 50, status: "working".into() };
//!     let validated = validate_channel_message::<ProgressUpdate>(
//!         serde_json::to_value(&msg).unwrap()
//!     )?;
//!     channel.send(serde_json::to_value(&validated).unwrap()).unwrap();
//!     Ok(())
//! }
//! ```
//!
//! # Frontend usage (TypeScript)
//!
//! ```javascript,ignore
//! import { invoke } from '@tauri-apps/api/core';
//!
//! interface VldError {
//!   error: string;
//!   issues: Array<{ path: string; message: string }>;
//! }
//!
//! try {
//!   const result = await invoke('create_user', {
//!     payload: { name: 'Alice', email: 'alice@example.com' }
//!   });
//! } catch (err) {
//!   const vldErr = err as VldError;
//!   for (const issue of vldErr.issues) {
//!     console.error(`${issue.path}: ${issue.message}`);
//!   }
//! }
//! ```

use serde::{Deserialize, Serialize};
use vld::schema::VldParse;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Single validation issue, serialised for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TauriIssue {
    /// JSON-path of the failing field, e.g. `.name`.
    pub path: String,
    /// Human-readable error message.
    pub message: String,
}

/// Serializable error type for Tauri IPC commands, events, and channel
/// messages.
///
/// Tauri requires command error types to implement `Serialize`.
/// `VldTauriError` serialises as:
///
/// ```json
/// {
///   "error": "Validation failed",
///   "issues": [
///     { "path": ".name", "message": "String must be at least 2 characters" }
///   ]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VldTauriError {
    /// Error category string (e.g. `"Validation failed"`, `"Invalid JSON"`).
    pub error: String,
    /// List of individual validation issues.
    pub issues: Vec<TauriIssue>,
}

impl VldTauriError {
    /// Create from a [`VldError`](vld::error::VldError).
    pub fn from_vld(e: &vld::error::VldError) -> Self {
        let issues = e
            .issues
            .iter()
            .map(|issue| {
                let path: String = issue
                    .path
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(".");
                TauriIssue {
                    path,
                    message: issue.message.clone(),
                }
            })
            .collect();
        Self {
            error: "Validation failed".into(),
            issues,
        }
    }

    /// Build a JSON-parse error.
    pub fn json_parse_error(msg: impl std::fmt::Display) -> Self {
        Self {
            error: "Invalid JSON".into(),
            issues: vec![TauriIssue {
                path: String::new(),
                message: msg.to_string(),
            }],
        }
    }

    /// Build a generic error with a custom category.
    pub fn custom(error: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            issues: vec![TauriIssue {
                path: String::new(),
                message: message.into(),
            }],
        }
    }

    /// `true` if the error contains any issues.
    pub fn has_issues(&self) -> bool {
        !self.issues.is_empty()
    }

    /// Number of issues.
    pub fn issue_count(&self) -> usize {
        self.issues.len()
    }
}

impl std::fmt::Display for VldTauriError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {} issue(s)", self.error, self.issues.len())
    }
}

impl std::error::Error for VldTauriError {}

impl From<vld::error::VldError> for VldTauriError {
    fn from(e: vld::error::VldError) -> Self {
        Self::from_vld(&e)
    }
}

// ---------------------------------------------------------------------------
// Core validate function
// ---------------------------------------------------------------------------

/// Validate a `serde_json::Value` against a `vld` schema.
///
/// General-purpose entry-point used by all specialised helpers.
///
/// # Example
///
/// ```rust
/// use vld_tauri::validate;
///
/// vld::schema! {
///     #[derive(Debug)]
///     struct Greet {
///         name: String => vld::string().min(1),
///     }
/// }
///
/// let val = serde_json::json!({"name": "Alice"});
/// let g = validate::<Greet>(val).unwrap();
/// assert_eq!(g.name, "Alice");
/// ```
pub fn validate<T: VldParse>(value: serde_json::Value) -> Result<T, VldTauriError> {
    T::vld_parse_value(&value).map_err(VldTauriError::from)
}

/// Validate a raw JSON string against a `vld` schema.
///
/// Convenience wrapper — parses the string first, then validates.
///
/// # Example
///
/// ```rust
/// use vld_tauri::validate_args;
///
/// vld::schema! {
///     #[derive(Debug)]
///     struct Ping {
///         msg: String => vld::string().min(1),
///     }
/// }
///
/// let p = validate_args::<Ping>(r#"{"msg":"pong"}"#).unwrap();
/// assert_eq!(p.msg, "pong");
/// ```
pub fn validate_args<T: VldParse>(json_str: &str) -> Result<T, VldTauriError> {
    let value: serde_json::Value =
        serde_json::from_str(json_str).map_err(VldTauriError::json_parse_error)?;
    validate(value)
}

// ---------------------------------------------------------------------------
// Specialised validators (semantically clear aliases)
// ---------------------------------------------------------------------------

/// Validate an **event payload** received from the frontend via
/// `Emitter::emit()` / `Listener::listen()`.
///
/// Functionally identical to [`validate`], but conveys intent.
///
/// # Example
///
/// ```rust,ignore
/// app.listen("user:update", |event| {
///     let payload: serde_json::Value = serde_json::from_str(event.payload()).unwrap();
///     let update = validate_event::<UserUpdate>(payload).unwrap();
/// });
/// ```
pub fn validate_event<T: VldParse>(payload: serde_json::Value) -> Result<T, VldTauriError> {
    validate(payload)
}

/// Validate **app state / configuration** before calling `app.manage()`.
///
/// Returns the validated value ready to be managed.
///
/// # Example
///
/// ```rust
/// use vld_tauri::validate_state;
///
/// vld::schema! {
///     #[derive(Debug)]
///     struct AppConfig {
///         db_url: String => vld::string().min(1),
///         max_connections: i64 => vld::number().int().min(1).max(100),
///     }
/// }
///
/// let cfg = serde_json::json!({"db_url": "postgres://...", "max_connections": 10});
/// let config = validate_state::<AppConfig>(cfg).unwrap();
/// assert_eq!(config.max_connections, 10);
/// ```
pub fn validate_state<T: VldParse>(value: serde_json::Value) -> Result<T, VldTauriError> {
    validate(value)
}

/// Validate a **Tauri plugin configuration** (usually from `tauri.conf.json`).
///
/// # Example
///
/// ```rust
/// use vld_tauri::validate_plugin_config;
///
/// vld::schema! {
///     #[derive(Debug)]
///     struct MyPluginConfig {
///         api_key: String => vld::string().min(1),
///         timeout: i64    => vld::number().int().min(100),
///     }
/// }
///
/// let cfg = serde_json::json!({"api_key": "abc123", "timeout": 5000});
/// let c = validate_plugin_config::<MyPluginConfig>(cfg).unwrap();
/// assert_eq!(c.api_key, "abc123");
/// ```
pub fn validate_plugin_config<T: VldParse>(config: serde_json::Value) -> Result<T, VldTauriError> {
    validate(config)
}

/// Validate an outgoing **channel message** before sending it to the
/// frontend via `Channel::send()`.
///
/// Useful for ensuring the backend only emits well-formed data.
///
/// # Example
///
/// ```rust
/// use vld_tauri::validate_channel_message;
///
/// vld::schema! {
///     #[derive(Debug)]
///     struct Progress {
///         percent: i64  => vld::number().int().min(0).max(100),
///         status: String => vld::string().min(1),
///     }
/// }
///
/// let msg = serde_json::json!({"percent": 42, "status": "downloading"});
/// let p = validate_channel_message::<Progress>(msg).unwrap();
/// assert_eq!(p.percent, 42);
/// ```
pub fn validate_channel_message<T: VldParse>(
    message: serde_json::Value,
) -> Result<T, VldTauriError> {
    validate(message)
}

// ---------------------------------------------------------------------------
// VldPayload<T> — auto-validating wrapper for command args
// ---------------------------------------------------------------------------

/// Auto-validating wrapper for Tauri **command parameters**.
///
/// Implements [`Deserialize`] — on deserialization the incoming JSON is
/// validated through `T::vld_parse_value()`. If validation fails,
/// deserialization returns a `serde::de::Error` with the full message.
///
/// Implements [`Deref`](std::ops::Deref) to `T` so fields are accessible
/// directly.
///
/// # Example
///
/// ```rust,ignore
/// #[tauri::command]
/// fn greet(payload: VldPayload<GreetArgs>) -> Result<String, VldTauriError> {
///     Ok(format!("Hello, {}!", payload.name))
/// }
/// ```
pub struct VldPayload<T>(pub T);

impl<T> std::ops::Deref for VldPayload<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> std::ops::DerefMut for VldPayload<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for VldPayload<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("VldPayload").field(&self.0).finish()
    }
}

impl<T: Clone> Clone for VldPayload<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<'de, T: VldParse> Deserialize<'de> for VldPayload<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        T::vld_parse_value(&value).map(VldPayload).map_err(|e| {
            let msg = format_vld_error_inline(&e);
            serde::de::Error::custom(msg)
        })
    }
}

// ---------------------------------------------------------------------------
// VldEvent<T> — auto-validating wrapper for event payloads
// ---------------------------------------------------------------------------

/// Auto-validating wrapper for Tauri **event payloads**.
///
/// Works the same as [`VldPayload<T>`] but is semantically intended for
/// data received via `Listener::listen()`.
///
/// # Example
///
/// ```rust,ignore
/// app.listen("settings:changed", |event| {
///     if let Ok(s) = serde_json::from_str::<VldEvent<Settings>>(event.payload()) {
///         println!("New settings: {:?}", s.0);
///     }
/// });
/// ```
pub struct VldEvent<T>(pub T);

impl<T> std::ops::Deref for VldEvent<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> std::ops::DerefMut for VldEvent<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for VldEvent<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("VldEvent").field(&self.0).finish()
    }
}

impl<T: Clone> Clone for VldEvent<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<'de, T: VldParse> Deserialize<'de> for VldEvent<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        T::vld_parse_value(&value).map(VldEvent).map_err(|e| {
            let msg = format_vld_error_inline(&e);
            serde::de::Error::custom(msg)
        })
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn format_vld_error_inline(e: &vld::error::VldError) -> String {
    let issues: Vec<String> = e
        .issues
        .iter()
        .map(|i| {
            let path: String = i
                .path
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(".");
            format!("{path}: {}", i.message)
        })
        .collect();
    format!("Validation failed: {}", issues.join("; "))
}

// ---------------------------------------------------------------------------
// Prelude
// ---------------------------------------------------------------------------

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{
        validate, validate_args, validate_channel_message, validate_event, validate_plugin_config,
        validate_state, TauriIssue, VldEvent, VldPayload, VldTauriError,
    };
    pub use vld::prelude::*;
}
