//! Helpers shared by the modules of the `it` test binary.
#![allow(dead_code)]

use vld::error::VldError;
use vld_clap::{ErrorSource, VldClapError};

/// `(path, code)` of every validation issue (`.port`); panics on `Ok` and
/// on serialization errors.
#[track_caller]
pub fn issues<T: std::fmt::Debug>(r: Result<T, VldClapError>) -> Vec<(String, String)> {
    match r {
        Err(VldClapError {
            source: ErrorSource::Validation(e),
            ..
        }) => e
            .issues
            .iter()
            .map(|i| {
                (
                    i.path.iter().map(ToString::to_string).collect(),
                    i.code.key().to_string(),
                )
            })
            .collect(),
        other => panic!("expected a validation error, got {other:?}"),
    }
}

/// Owned `(path, code)` pairs to compare with [`issues`].
pub fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
    expected
        .iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect()
}

/// The argument names in an error message: the text before `: ` on every
/// issue line.
pub fn arg_names(message: &str) -> Vec<&str> {
    message
        .lines()
        .skip(1)
        .map(|l| l.trim_start().split(": ").next().unwrap())
        .collect()
}

/// CLI args that fail with exactly the error they carry (`{"fail": …}` once
/// serialized) and pass when it has no issues; its clap command has one
/// argument of every naming kind. Lets a test put any issue list through
/// `validate_cli` / `validate` / `validate_json`.
#[derive(Debug, serde::Serialize)]
pub struct Echo {
    pub fail: VldError,
}

impl vld::schema::VldParse for Echo {
    fn vld_parse_value(value: &serde_json::Value) -> Result<Self, VldError> {
        let fail: VldError = serde_json::from_value(value["fail"].clone()).expect("a VldError");
        if fail.issues.is_empty() {
            Ok(Echo { fail })
        } else {
            Err(fail)
        }
    }
}

impl clap::CommandFactory for Echo {
    fn command() -> clap::Command {
        use clap::{Arg, Command};
        Command::new("app")
            .arg(Arg::new("output_file").long("out"))
            .arg(Arg::new("tag").short('t'))
            .arg(Arg::new("both").long("both").short('b'))
            .arg(Arg::new("max_connections").long("max-connections"))
            .arg(Arg::new("input"))
            .arg(Arg::new("files").value_name("FILE").num_args(1..))
            .subcommand(Command::new("add").arg(Arg::new("name").long("name")))
    }
    fn command_for_update() -> clap::Command {
        Self::command()
    }
}

/// How [`Echo::command`] names each argument id.
pub const ECHO_ARGS: &[(&str, &str)] = &[
    ("output_file", "--out"),
    ("tag", "-t"),
    ("both", "--both"),
    ("max_connections", "--max-connections"),
    ("input", "<INPUT>"),
    ("files", "<FILE>"),
    ("name", "--name"),
];
