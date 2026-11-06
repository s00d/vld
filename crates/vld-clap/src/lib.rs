//! # vld-clap — validate clap arguments with `vld`
//!
//! clap parses types, defaults and required arguments; `vld-clap` checks the
//! values (formats, ranges, lengths) afterwards, using `#[derive(Validate)]`
//! rules on the same clap struct. No separate schema.
//!
//! ```rust
//! use clap::Parser;
//! use vld::Validate;
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
//!     /// Output file
//!     #[arg(long = "out")]
//!     #[vld(vld::string().min(1))]
//!     output_file: String,
//! }
//!
//! let cli = Cli::parse_from(["app", "--email", "nope", "--port", "0", "--out", "x"]);
//! let err = vld_clap::validate_cli(&cli).unwrap_err();
//! assert_eq!(
//!     err.message,
//!     "Invalid arguments:\n       --email: Invalid email address\n       --port: Number must be at least 1",
//! );
//! // In `main`: `vld_clap::validate_or_exit(&cli)` prints this and exits with code 2.
//! ```
//!
//! # Flag names in errors
//!
//! [`validate_cli`] and [`validate_or_exit`] (for `clap::Parser` /
//! `CommandFactory` types) name each argument the way clap does:
//! `#[arg(long = "out")] output_file` is `--out`, a positional `input` is
//! `<INPUT>`. [`validate`] works on any `Serialize` struct and guesses
//! `--kebab-case` from the field name.
//!
//! Guide: <https://s00d.github.io/vld/integrations/config-cli/clap>

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

    /// Format each issue on its own line, prefixed with the flag name
    /// (as in [`message`](Self::message)).
    pub fn format_issues(&self) -> String {
        match &self.source {
            ErrorSource::Validation(_) => self
                .message
                .lines()
                .skip(1)
                .map(|l| format!("  {}", l.trim_start()))
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

/// `--flag-name: message`. With a clap `Command` the field's argument is
/// looked up (`--out`, `-o`, `<INPUT>`); otherwise clap's default kebab-case
/// flag name is assumed (`max_connections` -> `--max-connections`, nested
/// `user.email` -> `--user.email`).
fn issue_line(i: &vld::error::ValidationIssue, cmd: Option<&clap::Command>) -> String {
    use vld::error::PathSegment;
    // The outermost path field that names an argument of the command, a
    // flattened group (already merged into the command) or a subcommand.
    let arg = cmd.and_then(|cmd| {
        i.path.iter().enumerate().find_map(|(n, seg)| match seg {
            PathSegment::Field(id) => find_arg(cmd, id).map(|a| (n, a)),
            _ => None,
        })
    });
    let field = match arg {
        Some((n, arg)) => {
            let rest = vld_integration_common::format_path(&i.path[n + 1..]);
            let name = if let Some(long) = arg.get_long() {
                format!("--{long}")
            } else if let Some(short) = arg.get_short() {
                format!("-{short}")
            } else {
                let value = arg
                    .get_value_names()
                    .and_then(|v| v.first())
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| arg.get_id().as_str().to_uppercase());
                format!("<{value}>")
            };
            // `<FILES>[0]`, `--tag[0]`, `--db.url`
            let sep = if rest.is_empty() || rest.starts_with('[') {
                ""
            } else {
                "."
            };
            format!("{name}{sep}{rest}")
        }
        None => {
            let field = vld_integration_common::format_path(&i.path).replace('_', "-");
            if field.is_empty() {
                return i.message.clone();
            }
            format!("--{field}")
        }
    };
    format!("{}: {}", field, i.message)
}

/// Argument `id` of `cmd` or of any (nested) subcommand.
fn find_arg<'a>(cmd: &'a clap::Command, id: &str) -> Option<&'a clap::Arg> {
    cmd.get_arguments()
        .find(|a| a.get_id() == id)
        .or_else(|| cmd.get_subcommands().find_map(|s| find_arg(s, id)))
}

fn make_error(e: vld::error::VldError) -> VldClapError {
    make_error_for(e, None)
}

fn make_error_for(e: vld::error::VldError, cmd: Option<&clap::Command>) -> VldClapError {
    let summary = e
        .issues
        .iter()
        .map(|i| issue_line(i, cmd))
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

/// Like [`validate`], for a clap `Parser`: errors name each argument as
/// clap does (`#[arg(long = "out")]` → `--out`, positional → `<INPUT>`).
pub fn validate_cli<T>(args: &T) -> Result<(), VldClapError>
where
    T: VldParse + serde::Serialize + clap::CommandFactory,
{
    match validate(args) {
        Err(VldClapError {
            source: ErrorSource::Validation(e),
            ..
        }) => Err(make_error_for(e, Some(&T::command()))),
        other => other,
    }
}

/// Validate and exit on failure — convenience wrapper.
///
/// Calls [`validate_cli`] and, on error, prints the error to stderr
/// and exits with code 2.
///
/// ```rust,ignore
/// let cli = Cli::parse();
/// vld_clap::validate_or_exit(&cli);
/// println!("All good: {:?}", cli);
/// ```
pub fn validate_or_exit<T>(args: &T)
where
    T: VldParse + serde::Serialize + clap::CommandFactory,
{
    if let Err(e) = validate_cli(args) {
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
        validate, validate_cli, validate_json, validate_or_exit, validate_with_schema, VldClapError,
    };
    pub use vld::prelude::*;
}
