//! `vld_app::config` behaviour tests.
//!
//! | File | Kind | What |
//! |---|---|---|
//! | `story_config_rs.rs` | stories (feature `config-rs`) | A config file (TOML / JSON) with a nested section, layered with env vars, loaded through `from_builder` / `from_config`: typed values, env text coercion, issue paths of a broken file, source errors. |
//! | `story_figment.rs` | stories (feature `figment`) | The same through figment: defaults, TOML file and env layers, empty figment, extraction errors. |
//! | `story_value.rs` | stories | `from_value` (strict) and `from_value_coerced` on an already loaded JSON value; the error type. |
//! | `model_env.rs` | model (proptest, feature `config-rs`) | Random env text values (words, integers with signs / leading zeros, booleans in any case) through config-rs `try_parsing` and `from_builder`: a model predicts every field value or the exact `(path, code)` list; `from_value_coerced` agrees. |
//!
//! Compared: values, issue paths and codes; message texts only for `Display`.

mod common;

#[cfg(feature = "config-rs")]
mod model_env;
#[cfg(feature = "config-rs")]
mod story_config_rs;
#[cfg(feature = "figment")]
mod story_figment;
mod story_value;
