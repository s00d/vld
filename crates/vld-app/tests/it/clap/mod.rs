//! `vld_app::clap` behaviour tests.
//!
//! | File | Kind | What |
//! |---|---|---|
//! | `story_cli.rs` | stories | A clap `Parser` struct with `#[derive(Validate)]` rules: args parsed with `parse_from`, validated with `validate_cli` / `validate`; the error message names each argument as clap does (long, short, positional, list index, flattened group, subcommand); `validate_json` / `validate_with_schema` return typed values; serialization failures. |
//! | `story_exit.rs` | stories | `validate_or_exit` in a child process: exit code 2 and the `error: …` text on stderr; valid args continue. |
//! | `model_flags.rs` | model (proptest) | Random issue paths and messages against a command with long / short / positional / value-name / subcommand args: a model predicts every line of the message and of `format_issues`, with and without the clap command. |
//!
//! Compared: values, issue paths / codes, the argument names in the message;
//! full message texts only in the stories about the printed message.

mod common;

mod model_flags;
mod story_cli;
mod story_exit;
