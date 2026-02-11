//! # vld-clap — Clap integration for `vld`
//!
//! Validate CLI arguments **after** `clap` has parsed them, using
//! `#[derive(Validate)]` directly on the clap struct. No separate schema
//! needed.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use clap::Parser;
//! use vld::Validate;
//! use vld_clap::prelude::*;
//!
//! #[derive(Parser, Debug, serde::Serialize, Validate)]
//! struct Cli {
//!     /// Admin email address
//!     #[arg(long)]
//!     #[vld(vld::string().email())]
//!     email: String,
//!
//!     /// Server port
//!     #[arg(long, default_value_t = 8080)]
//!     #[vld(vld::number().int().min(1).max(65535))]
//!     port: i64,
//!
//!     /// Application name
//!     #[arg(long)]
//!     #[vld(vld::string().min(2).max(50))]
//!     name: String,
//! }
//!
//! fn main() {
//!     let cli = Cli::parse(); // clap's parse — no conflict
//!     validate_or_exit(&cli);
//!     println!("Valid! {:?}", cli);
//! }
//! ```

use std::fmt;
use vld::schema::VldParse;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error returned by `vld-clap` validation functions.
#[derive(Debug, Clone)]
pub struct VldClapError {
    /// The underlying vld validation error (if any).
    pub source: ErrorSource,
    /// Human-readable summary of all issues for CLI display.
    pub message: String,
}

/// Source of the error.
#[derive(Debug, Clone)]
pub enum ErrorSource {
    /// Validation failed.
    Validation(vld::error::VldError),
    /// Serialization to JSON failed.
    Serialization(String),
}

impl VldClapError {
    /// Print the error to stderr and exit with code 2 (standard for usage errors).
    pub fn exit(&self) -> ! {
        eprintln!("error: {}", self.message);
        std::process::exit(2);
    }

    /// Format each issue on its own line, prefixed with `--field`.
    pub fn format_issues(&self) -> String {
        match &self.source {
            ErrorSource::Validation(e) => e
                .issues
                .iter()
                .map(|i| {
                    let path = i
                        .path
                        .iter()
                        .map(|p| p.to_string())
                        .collect::<Vec<_>>()
                        .join(".");
                    let field = path.trim_start_matches('.');
                    if field.is_empty() {
                        format!("  {}", i.message)
                    } else {
                        format!("  --{}: {}", field, i.message)
                    }
                })
                .collect::<Vec<_>>()
                .join("\n"),
            ErrorSource::Serialization(msg) => format!("  {}", msg),
        }
    }
}

impl fmt::Display for VldClapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for VldClapError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.source {
            ErrorSource::Validation(e) => Some(e),
            ErrorSource::Serialization(_) => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Core validation functions
// ---------------------------------------------------------------------------

fn make_error(e: vld::error::VldError) -> VldClapError {
    let summary = e
        .issues
        .iter()
        .map(|i| {
            let path = i
                .path
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(".");
            let field = path.trim_start_matches('.');
            if field.is_empty() {
                i.message.clone()
            } else {
                format!("--{}: {}", field, i.message)
            }
        })
        .collect::<Vec<_>>()
        .join("\n       ");
    VldClapError {
        message: format!("Invalid arguments:\n       {}", summary),
        source: ErrorSource::Validation(e),
    }
}

/// Validate a parsed CLI struct that implements `#[derive(Validate)]` + `Serialize`.
///
/// The struct is serialized to JSON, then validated via the vld rules
/// defined by `#[vld(...)]` attributes on its fields.
///
/// # Example
///
/// ```rust
/// use vld::Validate;
///
/// #[derive(Debug, serde::Serialize, Validate)]
/// struct Args {
///     #[vld(vld::number().int().min(1).max(65535))]
///     port: i64,
///     #[vld(vld::string().min(1))]
///     host: String,
/// }
///
/// let args = Args { port: 8080, host: "localhost".into() };
/// assert!(vld_clap::validate(&args).is_ok());
///
/// let bad = Args { port: 0, host: "".into() };
/// assert!(vld_clap::validate(&bad).is_err());
/// ```
pub fn validate<T>(args: &T) -> Result<(), VldClapError>
where
    T: VldParse + serde::Serialize,
{
    let json = serde_json::to_value(args).map_err(|e| VldClapError {
        source: ErrorSource::Serialization(e.to_string()),
        message: format!("Failed to serialize arguments: {}", e),
    })?;
    T::vld_parse_value(&json).map(|_| ()).map_err(make_error)
}

/// Validate and exit on failure — convenience wrapper.
///
/// Calls [`validate`] and, on error, prints the error to stderr
/// and exits with code 2.
///
/// ```rust,ignore
/// let cli = Cli::parse();
/// vld_clap::validate_or_exit(&cli);
/// println!("All good: {:?}", cli);
/// ```
pub fn validate_or_exit<T>(args: &T)
where
    T: VldParse + serde::Serialize,
{
    if let Err(e) = validate(args) {
        e.exit();
    }
}

/// Validate a JSON value against any `VldParse` schema `S`.
///
/// Useful when the CLI args are already a JSON object.
pub fn validate_json<S>(json: &serde_json::Value) -> Result<S, VldClapError>
where
    S: VldParse,
{
    S::vld_parse_value(json).map_err(make_error)
}

/// Validate a `Serialize`-able value against any `VldParse` schema `S`.
///
/// Use this when you have a separate schema and a data struct (the old
/// pattern). Prefer [`validate`] with `#[derive(Validate)]` for the
/// idiomatic approach.
pub fn validate_with_schema<S, T>(args: &T) -> Result<S, VldClapError>
where
    S: VldParse,
    T: serde::Serialize,
{
    let json = serde_json::to_value(args).map_err(|e| VldClapError {
        source: ErrorSource::Serialization(e.to_string()),
        message: format!("Failed to serialize arguments: {}", e),
    })?;
    S::vld_parse_value(&json).map_err(make_error)
}

// ---------------------------------------------------------------------------
// Prelude
// ---------------------------------------------------------------------------

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{
        validate, validate_json, validate_or_exit, validate_with_schema, VldClapError,
    };
    pub use vld::prelude::*;
}
