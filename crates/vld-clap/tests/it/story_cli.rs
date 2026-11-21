//! Stories: a CLI parses its args with clap and checks them with vld.

use crate::common::*;

use clap::Parser;
use std::error::Error;
use vld::Validate;
use vld_clap::prelude::*;
use vld_clap::{ErrorSource, VldClapError};

#[derive(Parser, Debug, serde::Serialize, Validate)]
struct ServerCli {
    #[arg(long)]
    #[vld(vld::string().min(1))]
    host: String,
    #[arg(long, default_value_t = 8080)]
    #[vld(vld::number().int().min(1).max(65535))]
    port: i64,
    #[arg(long, default_value_t = 4)]
    #[vld(vld::number().int().min(1).max(256))]
    workers: i64,
}

/// Given a server CLI whose clap struct carries vld rules,
/// when it is run with valid args (defaults, inclusive bounds) and with three
/// bad ones,
/// then valid args pass; bad ones give one error: an `Invalid arguments:`
/// header, one indented `--flag: message` line per issue (as `Display`
/// too), `format_issues` with the same lines indented by two, the
/// `VldError` as `source()` with each issue's path and code.
#[test]
fn server_cli_parsed_and_validated() {
    let cli = ServerCli::parse_from(["srv", "--host", "0.0.0.0"]);
    assert_eq!((cli.port, cli.workers), (8080, 4));
    validate_cli(&cli).unwrap();
    let edge = ServerCli::parse_from(["srv", "--host", "h", "--port", "65535", "--workers", "256"]);
    validate_cli(&edge).unwrap();

    let bad = ServerCli::parse_from(["srv", "--host", "", "--port", "99999", "--workers", "0"]);
    let err = validate_cli(&bad).unwrap_err();
    assert_eq!(arg_names(&err.message), ["--host", "--port", "--workers"]);
    let lines: Vec<&str> = err.message.lines().collect();
    assert_eq!(lines[0], "Invalid arguments:");
    assert!(
        lines[1..].iter().all(|l| l.starts_with("       --")),
        "{lines:?}"
    );
    assert_eq!(err.to_string(), err.message);
    assert_eq!(
        err.format_issues(),
        lines[1..]
            .iter()
            .map(|l| format!("  {}", l.trim_start()))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let source = err.source().expect("the VldError");
    assert!(source.downcast_ref::<vld::error::VldError>().is_some());
    assert_eq!(
        issues(validate_cli(&bad)),
        pairs(&[
            (".host", "too_small"),
            (".port", "too_big"),
            (".workers", "too_small")
        ])
    );
}

#[derive(Parser, Debug, serde::Serialize, Validate)]
struct Cli {
    #[arg(long)]
    #[vld(vld::number().int().min(1).max(64))]
    max_connections: i64,
    #[arg(long = "out")]
    #[vld(vld::string().ends_with(".json"))]
    output_file: String,
    #[arg(short = 't')]
    #[vld(vld::string().min(2))]
    tag: String,
    #[vld(vld::string().min(3))]
    input: String,
}

/// Given a CLI with a renamed long flag, a short-only flag and a positional,
/// when every arg fails,
/// then `validate_cli` names them as clap does (`--out`, `-t`, `<INPUT>`)
/// while `validate`, which has no clap command, guesses `--kebab-case` from
/// the field names.
#[test]
fn errors_use_clap_argument_names() {
    let cli = Cli::parse_from([
        "mycli",
        "--max-connections",
        "0",
        "--out",
        "x.txt",
        "-t",
        "a",
        "ab",
    ]);
    let err = validate_cli(&cli).unwrap_err();
    assert_eq!(
        arg_names(&err.message),
        ["--max-connections", "--out", "-t", "<INPUT>"]
    );
    assert!(
        err.format_issues().contains("  --out: "),
        "{}",
        err.format_issues()
    );

    let err = validate(&cli).unwrap_err();
    assert_eq!(
        arg_names(&err.message),
        ["--max-connections", "--output-file", "--tag", "--input"]
    );
}

#[derive(clap::Args, Debug, serde::Serialize, Validate)]
struct DbArgs {
    #[arg(long = "db-url")]
    #[vld(vld::string().min(5))]
    url: String,
}

#[derive(clap::Args, Debug, serde::Serialize, Validate)]
struct AddArgs {
    #[arg(long)]
    #[vld(vld::string().min(3))]
    name: String,
}

#[derive(clap::Subcommand, Debug, serde::Serialize)]
#[serde(untagged)]
enum Cmd {
    Add(AddArgs),
}

#[derive(Parser, Debug, serde::Serialize, Validate)]
struct Deep {
    #[vld(vld::array(vld::string().min(2)))]
    files: Vec<String>,
    #[arg(long)]
    #[vld(vld::array(vld::string().min(2)))]
    tag: Vec<String>,
    #[command(flatten)]
    #[vld(vld::nested(<DbArgs as vld::schema::VldParse>::vld_parse_value))]
    db: DbArgs,
    #[command(subcommand)]
    #[vld(vld::nested(|v: &serde_json::Value| {
        <AddArgs as vld::schema::VldParse>::vld_parse_value(v).map(Cmd::Add)
    }))]
    cmd: Cmd,
}

/// Given a CLI with list args, a flattened group and a subcommand,
/// when an item of each list, the group's flag and the subcommand's flag fail,
/// then each line names the clap argument, with the index on lists
/// (`<FILES>[0]`, `--tag[0]`) and the inner flag of the group / subcommand.
#[test]
fn lists_flattened_groups_and_subcommands() {
    let cli = Deep::parse_from([
        "mycli", "a", "ok", "--tag", "b", "--db-url", "x", "add", "--name", "n",
    ]);
    let err = validate_cli(&cli).unwrap_err();
    assert_eq!(
        arg_names(&err.message),
        ["<FILES>[0]", "--tag[0]", "--db-url", "--name"]
    );

    let good = Deep::parse_from([
        "mycli", "aa", "--tag", "bb", "--db-url", "pg://x", "add", "--name", "neo",
    ]);
    validate_cli(&good).unwrap();
}

#[derive(Debug, serde::Serialize, Validate)]
struct UserCli {
    #[vld(vld::string().email())]
    email: String,
    #[vld(vld::string().min(2).max(100))]
    name: String,
    #[vld(vld::number().int().min(0).max(150))]
    age: i64,
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct PortSchema {
        pub port: i64 => vld::number().int().min(1).max(65535),
    }
}

/// Given args that are a plain `Serialize` struct (no clap), args checked
/// against a separate schema, and args already in JSON,
/// when they are valid or not,
/// then `validate` / `validate_with_schema` / `validate_json` accept the
/// valid ones (returning the schema's typed value) and report the bad ones
/// by path and code with `--kebab-case` names.
#[test]
fn plain_structs_separate_schemas_and_json() {
    let user = UserCli {
        email: "alice@example.com".into(),
        name: "Alice".into(),
        age: 30,
    };
    validate(&user).unwrap();
    assert_eq!(
        issues(validate(&UserCli { age: 151, ..user })),
        pairs(&[(".age", "too_big")])
    );
    let bad = UserCli {
        email: "bad".into(),
        name: "X".into(),
        age: -5,
    };
    assert_eq!(
        issues(validate(&bad)),
        pairs(&[
            (".email", "invalid_string"),
            (".name", "too_small"),
            (".age", "too_small")
        ])
    );
    assert_eq!(
        arg_names(&validate(&bad).unwrap_err().message),
        ["--email", "--name", "--age"]
    );

    #[derive(serde::Serialize)]
    struct Args {
        port: i64,
    }
    assert_eq!(
        validate_with_schema::<PortSchema, _>(&Args { port: 3000 })
            .unwrap()
            .port,
        3000
    );
    let r = validate_with_schema::<PortSchema, _>(&Args { port: 0 });
    assert_eq!(arg_names(&r.as_ref().unwrap_err().message), ["--port"]);
    assert_eq!(issues(r), pairs(&[(".port", "too_small")]));

    let json = serde_json::json!({"email": "a@b.co", "name": "Al", "age": 0});
    let u = validate_json::<UserCli>(&json).unwrap();
    assert_eq!(
        (u.email.as_str(), u.name.as_str(), u.age),
        ("a@b.co", "Al", 0)
    );
    let r = validate_json::<PortSchema>(&serde_json::json!({"port": 70000}));
    assert_eq!(arg_names(&r.as_ref().unwrap_err().message), ["--port"]);
    assert_eq!(issues(r), pairs(&[(".port", "too_big")]));
}

/// Args whose serialization fails.
#[derive(Debug)]
struct Unserializable;

impl serde::Serialize for Unserializable {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("no way"))
    }
}

impl vld::schema::VldParse for Unserializable {
    fn vld_parse_value(_: &serde_json::Value) -> Result<Self, vld::error::VldError> {
        Ok(Unserializable)
    }
}

impl clap::CommandFactory for Unserializable {
    fn command() -> clap::Command {
        clap::Command::new("app")
    }
    fn command_for_update() -> clap::Command {
        Self::command()
    }
}

/// Given args that cannot be serialized to JSON,
/// when they are validated through each entry point,
/// then the error is a serialization error (not a validation one): the
/// serializer's message, `Failed to serialize arguments: …` as the message,
/// `format_issues` indenting it, no `source()`.
#[test]
fn unserializable_args_are_a_serialization_error() {
    let check = |err: VldClapError| {
        let ErrorSource::Serialization(msg) = &err.source else {
            panic!("{err:?}")
        };
        assert_eq!(msg, "no way");
        assert_eq!(err.message, "Failed to serialize arguments: no way");
        assert_eq!(err.to_string(), err.message);
        assert_eq!(err.format_issues(), "  no way");
        assert!(err.source().is_none());
    };
    check(validate(&Unserializable).unwrap_err());
    check(validate_cli(&Unserializable).unwrap_err());
    check(validate_with_schema::<PortSchema, _>(&Unserializable).unwrap_err());
}
