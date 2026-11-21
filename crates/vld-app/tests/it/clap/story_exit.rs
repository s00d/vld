//! Stories: `validate_or_exit` in `main` — the process exits with code 2.
//!
//! The test re-runs its own test binary as the CLI process: with
//! `VLD_CLAP_EXIT_CHILD` set the test body is the CLI's `main`.

use clap::Parser;
use std::process::Command;
use vld::Validate;

#[derive(Parser, Debug, serde::Serialize, Validate)]
struct Cli {
    #[arg(long)]
    #[vld(vld::string().email())]
    email: String,
    #[arg(long, default_value_t = 8080)]
    #[vld(vld::number().int().min(1).max(65535))]
    port: i64,
}

const CHILD: &str = "VLD_CLAP_EXIT_CHILD";

fn args(kind: &str) -> Vec<&'static str> {
    match kind {
        "bad" => vec!["app", "--email", "nope", "--port", "0"],
        _ => vec!["app", "--email", "a@b.co"],
    }
}

/// Runs this test in a child process as the CLI with `kind` args.
fn run_child(kind: &str) -> std::process::Output {
    Command::new(std::env::current_exe().unwrap())
        .args([
            "clap::story_exit::validate_or_exit_ends_the_process_on_bad_args",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD, kind)
        .output()
        .unwrap()
}

/// Given a CLI whose `main` calls `vld_app::clap::validate_or_exit(&cli)`,
/// when it is started with bad args,
/// then it prints `error: ` and the validation message to stderr and exits
/// with code 2; with good args `main` goes on.
#[test]
fn validate_or_exit_ends_the_process_on_bad_args() {
    if let Ok(kind) = std::env::var(CHILD) {
        // The CLI process.
        let cli = Cli::parse_from(args(&kind));
        vld_app::clap::validate_or_exit(&cli);
        println!("CLI CONTINUES");
        return;
    }

    let bad = run_child("bad");
    assert_eq!(bad.status.code(), Some(2));
    let expected = vld_app::clap::validate_cli(&Cli::parse_from(args("bad")))
        .unwrap_err()
        .message;
    let stderr = String::from_utf8(bad.stderr).unwrap();
    assert!(stderr.contains(&format!("error: {expected}\n")), "{stderr}");
    assert!(expected.starts_with("Invalid arguments:\n       --email: "));
    assert!(!String::from_utf8_lossy(&bad.stdout).contains("CLI CONTINUES"));

    let good = run_child("good");
    assert_eq!(good.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&good.stdout).contains("CLI CONTINUES"));
}
