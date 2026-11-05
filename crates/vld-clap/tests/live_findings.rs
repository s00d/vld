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
