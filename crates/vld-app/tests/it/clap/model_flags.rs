//! Model test: random issue paths and messages through the error formatter.
//!
//! proptest builds a `VldError` (0–5 issues; paths of argument ids, other
//! field names with underscores and list indexes, root included) and puts it
//! through [`Echo`] — whose clap command has a renamed long flag, a short-only
//! flag, a long+short flag, positionals with and without a value name and a
//! subcommand flag — with `validate_cli`, `validate` and `validate_json`.
//! A model predicts every message line:
//!
//! - with the command: the outermost path field naming an argument becomes
//!   its clap name (long, else short, else `<VALUE_NAME>` / `<ID>`), followed
//!   by the rest of the path (`[0]` directly, a field after a dot);
//! - without one (or no argument in the path): `--` and the dotted path with
//!   `_` → `-`; a root issue is the bare message;
//!
//! and the `format_issues` text, the `Ok` for an empty issue list and the
//! `(path, code)` list kept in `source`.

use super::common::*;

use proptest::prelude::*;
use vld::error::{IssueCode, PathSegment, ValidationIssue, VldError};
use vld_app::clap::{ErrorSource, validate, validate_cli, validate_json};

fn segment() -> impl Strategy<Value = PathSegment> {
    let fields = vec![
        "output_file",
        "tag",
        "both",
        "max_connections",
        "input",
        "files",
        "name",
        "db_url",
        "user",
        "x",
    ];
    prop_oneof![
        3 => prop::sample::select(fields).prop_map(|f| PathSegment::Field(f.into())),
        1 => (0usize..12).prop_map(PathSegment::Index),
    ]
}

fn model_path(path: &[PathSegment]) -> String {
    let s: String = path
        .iter()
        .map(|seg| match seg {
            PathSegment::Field(f) => format!(".{f}"),
            PathSegment::Index(i) => format!("[{i}]"),
        })
        .collect();
    s.strip_prefix('.').map(str::to_string).unwrap_or(s)
}

fn model_line(path: &[PathSegment], message: &str, with_command: bool) -> String {
    let arg = path.iter().enumerate().find_map(|(n, seg)| match seg {
        PathSegment::Field(id) if with_command => ECHO_ARGS
            .iter()
            .find(|(a, _)| a == id)
            .map(|(_, name)| (n, *name)),
        _ => None,
    });
    match arg {
        Some((n, name)) => {
            let rest = model_path(&path[n + 1..]);
            let sep = if rest.is_empty() || rest.starts_with('[') {
                ""
            } else {
                "."
            };
            format!("{name}{sep}{rest}: {message}")
        }
        None => match model_path(path).replace('_', "-") {
            p if p.is_empty() => message.to_string(),
            p => format!("--{p}: {message}"),
        },
    }
}

fn issues_strategy() -> impl Strategy<Value = Vec<(Vec<PathSegment>, String)>> {
    prop::collection::vec(
        (
            prop::collection::vec(segment(), 0..4),
            "[A-Za-z][A-Za-z0-9 .,]{0,15}",
        ),
        0..6,
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn message_lines_match_the_model(generated in issues_strategy()) {
        let fail = VldError {
            issues: generated
                .iter()
                .enumerate()
                .map(|(n, (path, message))| ValidationIssue {
                    code: IssueCode::Custom { code: format!("c{n}") },
                    message: message.clone(),
                    path: path.clone(),
                    received: None,
                })
                .collect(),
        };
        let echo = Echo { fail: fail.clone() };

        if generated.is_empty() {
            prop_assert!(validate_cli(&echo).is_ok());
            prop_assert!(validate(&echo).is_ok());
            prop_assert!(validate_json::<Echo>(&serde_json::to_value(&echo).unwrap()).is_ok());
            return Ok(());
        }

        let json_err = validate_json::<Echo>(&serde_json::to_value(&echo).unwrap()).unwrap_err();
        for (err, with_command) in [
            (validate_cli(&echo).unwrap_err(), true),
            (validate(&echo).unwrap_err(), false),
            (json_err, false),
        ] {
            let lines: Vec<String> = generated
                .iter()
                .map(|(path, message)| model_line(path, message, with_command))
                .collect();
            prop_assert_eq!(&err.message, &format!("Invalid arguments:\n       {}", lines.join("\n       ")));
            prop_assert_eq!(
                err.format_issues(),
                lines.iter().map(|l| format!("  {l}")).collect::<Vec<_>>().join("\n")
            );
            let ErrorSource::Validation(kept) = &err.source else {
                panic!("{err:?}")
            };
            let codes: Vec<&str> = kept.issues.iter().map(|i| i.code.key()).collect();
            let expected: Vec<String> = (0..generated.len()).map(|n| format!("c{n}")).collect();
            prop_assert_eq!(codes, expected.iter().map(String::as_str).collect::<Vec<_>>());
            prop_assert_eq!(
                kept.issues.iter().map(|i| i.path.clone()).collect::<Vec<_>>(),
                generated.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>()
            );
        }
    }
}
