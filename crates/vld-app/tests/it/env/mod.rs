//! `vld_app::env` behaviour tests.
//!
//! | File | Kind | What |
//! |---|---|---|
//! | `model_vars.rs` | model (proptest) | Random env text (words, integers, leading zeros, booleans in any case, empty) for flat and `__`-nested keys, prefix in any case, unrelated variables mixed in, through `from_vars_prefixed`: a model predicts every field value or the exact `(variable, code)` list. |
//! | `story_env.rs` | stories | Settings from variables: prefix stripping, nesting with `__`, text kept where the schema wants text, repeats, skipped names, conflicts, errors named by variable, `Display`, the process environment (`from_env*`, read only). |
//! | `story_dotenv.rs` | stories (feature `dotenv`) | A `.env` file (comments, quotes, sections) through `from_dotenv`, layered under other variables with `read_dotenv`; missing file, syntax error, invalid values. |
//!
//! Compared: values, variable names and issue codes; message texts only for `Display`.
//! `std::env::set_var` is never called.

mod common;

mod model_vars;
#[cfg(feature = "dotenv")]
mod story_dotenv;
mod story_env;
