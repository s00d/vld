//! Regression tests for the live end-to-end findings (0.5.0).

use clap::Parser;
use vld::Validate;

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
    let err = vld_clap::validate_cli(&cli).unwrap_err();
    let msg = &err.message;
    assert!(msg.contains("--max-connections:"), "{msg}");
    assert!(msg.contains("--out:"), "{msg}");
    assert!(!msg.contains("--output-file"), "{msg}");
    assert!(msg.contains("-t:"), "{msg}");
    assert!(msg.contains("<INPUT>:"), "{msg}");
    assert!(!msg.contains("--input"), "{msg}");
    assert!(
        err.format_issues().contains("  --out:"),
        "{}",
        err.format_issues()
    );
}

// ---------------------------------------------------------------------------
// 0.5.0 re-audit: list indexes, flattened groups, subcommands.
// ---------------------------------------------------------------------------

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

#[test]
fn index_paths_flatten_and_subcommands() {
    let cli = Deep::parse_from([
        "mycli", "a", "--tag", "b", "--db-url", "x", "add", "--name", "n",
    ]);
    let err = vld_clap::validate_cli(&cli).unwrap_err();
    let msg = &err.message;
    assert!(msg.contains("<FILES>[0]:"), "{msg}");
    assert!(msg.contains("--tag[0]:"), "{msg}");
    assert!(!msg.contains(".["), "{msg}");
    assert!(msg.contains("--db-url:"), "{msg}");
    assert!(msg.contains("--name:"), "{msg}");
}
