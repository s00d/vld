//! # `vld_app::env` — validate configuration from environment variables
//!
//! Read environment variables (feature `env`, no dependency) or a `.env`
//! file (feature `dotenv`, [dotenvy](https://docs.rs/dotenvy) 0.15) into a
//! `vld` type at startup: a typed settings struct, or a [`VldEnvError`]
//! naming every invalid variable.
//!
//! # Quick Start
//!
//! ```rust
//! use vld_app::env::prelude::*;
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct Db {
//!         pub url: String     => vld::string().min(1),
//!         pub pool_size: i64  => vld::number().int().min(1).with_default(10),
//!     }
//! }
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct Settings {
//!         pub host: String => vld::string().min(1),
//!         pub port: i64    => vld::number().int().min(1).max(65535),
//!         pub debug: bool  => vld::boolean().with_default(false),
//!         pub db: Db       => vld::nested(Db::parse_value),
//!     }
//! }
//!
//! // In an application: `from_env_prefixed::<Settings>("APP_")`.
//! let vars = [
//!     ("APP_HOST", "0.0.0.0"),
//!     ("APP_PORT", "8080"),
//!     ("APP_DB__URL", "postgres://localhost/app"),
//!     ("PATH", "/usr/bin"), // no prefix: ignored
//! ];
//! let s: Settings = from_vars_prefixed(vars, "APP_").unwrap();
//! assert_eq!((s.host.as_str(), s.port, s.debug), ("0.0.0.0", 8080, false));
//! assert_eq!(s.db.pool_size, 10);
//!
//! let r: Result<Settings, _> = from_vars_prefixed([("APP_PORT", "http")], "APP_");
//! let err = r.unwrap_err();
//! let vars: Vec<&str> = err.var_issues().map(|(var, _)| var).collect();
//! assert_eq!(vars, ["APP_HOST", "APP_PORT", "APP_DB"]);
//! ```
//!
//! # Variable names
//!
//! - The prefix is matched ASCII case-insensitively and stripped; other
//!   variables are ignored. The rest of the name is lower-cased and split on
//!   `__` into nested objects: `APP_DB__POOL_SIZE` is `{"db": {"pool_size": ..}}`.
//!   Field names must therefore be lower snake case.
//! - Names with an empty part (`APP_`, `APP__X`, `APP_A__`) and, for the
//!   process environment, names or values that are not UTF-8 are skipped.
//! - When a name repeats, the later value wins (so in
//!   `read_dotenv(p)?.into_iter().chain(std::env::vars())` the process env
//!   overrides the file).
//! - A variable that is both a value and a section (`APP_DB` and
//!   `APP_DB__URL`) is a [`VldEnvError::Source`] error.
//!
//! # Values
//!
//! Each value is coerced with [`vld::schema::coerce_text`] (`8080` → number,
//! `true` → boolean, `01234` stays text) and the result parsed with
//! [`vld::schema::parse_coerced`], so a value follows its field's schema:
//! `APP_HOST=12345` into `vld::string()` is `"12345"`, and a single value
//! into `vld::array(..)` is a one-element array.
//!
//! Guide: <https://s00d.github.io/vld/integrations/config-cli/env>

use std::fmt;

use serde_json::{Map, Value};
use vld::error::{PathSegment, ValidationIssue, VldError};
use vld::schema::VldParse;

/// Error type for env validation.
#[derive(Debug)]
pub enum VldEnvError {
    /// Validation failed. `vars[i]` is the environment variable of
    /// `error.issues[i]` (see [`var_issues`](Self::var_issues)).
    Validation {
        /// Every validation issue; paths are JSON paths (`.db.url`).
        error: VldError,
        /// The variable each issue belongs to (`APP_DB__URL`).
        vars: Vec<String>,
    },
    /// The `.env` file could not be read.
    Io(std::io::Error),
    /// A `.env` syntax error, or a variable that is both a value and a
    /// section (`APP_DB` and `APP_DB__URL`).
    Source(String),
}

impl VldEnvError {
    /// `(variable, issue)` pairs of a validation error; empty otherwise.
    pub fn var_issues(&self) -> impl Iterator<Item = (&str, &ValidationIssue)> {
        let pairs = match self {
            VldEnvError::Validation { error, vars } => Some(vars.iter().zip(&error.issues)),
            _ => None,
        };
        pairs.into_iter().flatten().map(|(v, i)| (v.as_str(), i))
    }
}

impl fmt::Display for VldEnvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VldEnvError::Validation { .. } => {
                f.write_str("invalid environment:")?;
                for (var, issue) in self.var_issues() {
                    write!(f, "\n  {var}: {}", issue.message)?;
                }
                Ok(())
            }
            VldEnvError::Io(e) => write!(f, "env file error: {e}"),
            VldEnvError::Source(msg) => write!(f, "env error: {msg}"),
        }
    }
}

impl std::error::Error for VldEnvError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VldEnvError::Validation { error, .. } => Some(error),
            VldEnvError::Io(e) => Some(e),
            VldEnvError::Source(_) => None,
        }
    }
}

/// Validate the whole process environment (no prefix) into `T`.
///
/// Every variable is a candidate key, so prefer [`from_env_prefixed`].
pub fn from_env<T: VldParse>() -> Result<T, VldEnvError> {
    from_env_prefixed("")
}

/// Validate the process variables starting with `prefix` (stripped) into `T`.
pub fn from_env_prefixed<T: VldParse>(prefix: &str) -> Result<T, VldEnvError> {
    let vars = std::env::vars_os()
        .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)));
    from_vars_prefixed(vars, prefix)
}

/// Validate `(name, value)` pairs into `T`, without a prefix.
pub fn from_vars<T, I, K, V>(vars: I) -> Result<T, VldEnvError>
where
    T: VldParse,
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: AsRef<str>,
{
    from_vars_prefixed(vars, "")
}

/// Validate the `(name, value)` pairs starting with `prefix` (stripped) into
/// `T`. What [`from_env_prefixed`] does with the process environment.
pub fn from_vars_prefixed<T, I, K, V>(vars: I, prefix: &str) -> Result<T, VldEnvError>
where
    T: VldParse,
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: AsRef<str>,
{
    let mut root = Map::new();
    // (lower-cased key parts, original name) of every variable read.
    let mut names: Vec<(Vec<String>, String)> = Vec::new();
    for (name, value) in vars {
        let name = name.as_ref();
        let Some(rest) = strip_prefix(name, prefix) else {
            continue;
        };
        let parts: Vec<String> = rest
            .to_lowercase()
            .split("__")
            .map(str::to_string)
            .collect();
        if parts.iter().any(String::is_empty) {
            continue;
        }
        insert(&mut root, &parts, vld::schema::coerce_text(value.as_ref()))
            .map_err(|()| VldEnvError::Source(format!("{name} is both a value and a section")))?;
        match names.iter_mut().find(|(p, _)| *p == parts) {
            Some(entry) => entry.1 = name.to_string(),
            None => names.push((parts, name.to_string())),
        }
    }
    vld::schema::parse_coerced::<T>(&Value::Object(root)).map_err(|error| {
        let vars = error
            .issues
            .iter()
            .map(|i| var_name(&i.path, prefix, &names))
            .collect();
        VldEnvError::Validation { error, vars }
    })
}

/// Read a `.env` file into `(name, value)` pairs (in file order) without
/// touching the process environment. `${VAR}` references are expanded as
/// dotenvy does.
#[cfg(feature = "dotenv")]
pub fn read_dotenv(
    path: impl AsRef<std::path::Path>,
) -> Result<Vec<(String, String)>, VldEnvError> {
    let map_err = |e: dotenvy::Error| match e {
        dotenvy::Error::Io(e) => VldEnvError::Io(e),
        other => VldEnvError::Source(other.to_string()),
    };
    dotenvy::from_path_iter(path)
        .map_err(map_err)?
        .map(|r| r.map_err(map_err))
        .collect()
}

/// Validate a `.env` file (no prefix) into `T`, without touching the process
/// environment. To layer it under the process env, pass
/// `read_dotenv(path)?.into_iter().chain(std::env::vars())` to [`from_vars`].
#[cfg(feature = "dotenv")]
pub fn from_dotenv<T: VldParse>(path: impl AsRef<std::path::Path>) -> Result<T, VldEnvError> {
    from_vars(read_dotenv(path)?)
}

fn strip_prefix<'a>(name: &'a str, prefix: &str) -> Option<&'a str> {
    name.get(..prefix.len())
        .filter(|p| p.eq_ignore_ascii_case(prefix))
        .map(|_| &name[prefix.len()..])
}

/// Put `value` at `parts`; `Err` when a part is both a value and a section.
fn insert(root: &mut Map<String, Value>, parts: &[String], value: Value) -> Result<(), ()> {
    let (last, sections) = parts.split_last().expect("parts are never empty");
    let mut map = root;
    for part in sections {
        map = match map
            .entry(part.clone())
            .or_insert_with(|| Value::Object(Map::new()))
        {
            Value::Object(m) => m,
            _ => return Err(()),
        };
    }
    if matches!(map.get(last), Some(Value::Object(_))) {
        return Err(());
    }
    map.insert(last.clone(), value);
    Ok(())
}

/// The variable an issue path belongs to: the name read for its leading
/// fields (`.tags[0]` → `APP_TAGS`), else `PREFIX` + `FIELD__FIELD`.
fn var_name(path: &[PathSegment], prefix: &str, names: &[(Vec<String>, String)]) -> String {
    let fields: Vec<String> = path
        .iter()
        .map_while(|s| match s {
            PathSegment::Field(f) => Some(f.clone()),
            PathSegment::Index(_) => None,
        })
        .collect();
    names
        .iter()
        .find(|(p, _)| *p == fields)
        .map(|(_, n)| n.clone())
        .unwrap_or_else(|| format!("{prefix}{}", fields.join("__").to_uppercase()))
}

/// Prelude — re-exports everything you need.
pub mod prelude {
    pub use super::{VldEnvError, from_env, from_env_prefixed, from_vars, from_vars_prefixed};
    #[cfg(feature = "dotenv")]
    pub use super::{from_dotenv, read_dotenv};
    pub use vld::prelude::*;
}
