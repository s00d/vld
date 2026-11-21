//! Stories: settings from a `.env` file.

use super::common::*;

use std::io::Write;
use vld_app::env::prelude::*;

fn env_file(text: &str) -> tempfile::NamedTempFile {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(text.as_bytes()).unwrap();
    f
}

const FILE: &str = r#"
# app settings
HOST=localhost
PORT=3000
DEBUG=false
TAGS="a b"
DB__POOL_SIZE='5'
export DB__NAME=main
"#;

/// Given a `.env` file with comments, quotes, `export` and a section,
/// when it is validated with `from_dotenv`, and its pairs are read with
/// `read_dotenv` and layered under other variables,
/// then the file gives typed settings, the later variables override it, and
/// the process environment is untouched.
#[test]
fn dotenv_file_into_settings() {
    let f = env_file(FILE);
    let s: Settings = from_dotenv(f.path()).unwrap();
    assert_eq!(
        (s.host.as_str(), s.port, s.debug),
        ("localhost", 3000, false)
    );
    assert_eq!(s.tags, ["a b"]);
    assert_eq!(
        s.db,
        Db {
            pool_size: 5,
            name: "main".into()
        }
    );
    assert!(std::env::var("DB__NAME").is_err());

    let pairs = read_dotenv(f.path()).unwrap();
    assert_eq!(pairs[0], ("HOST".to_string(), "localhost".to_string()));
    let layered = pairs
        .into_iter()
        .chain([("PORT".to_string(), "9000".to_string())]);
    let s: Settings = from_vars(layered).unwrap();
    assert_eq!(s.port, 9000);
}

/// Given a missing file, a file with a syntax error and a file with bad
/// values,
/// when each is validated with `from_dotenv`,
/// then they are an `Io` error, a `Source` error and validation issues named
/// by the file's variables.
#[test]
fn dotenv_errors() {
    let dir = tempfile::tempdir().unwrap();
    let err = from_dotenv::<Settings>(dir.path().join("missing.env")).unwrap_err();
    assert!(
        matches!(&err, VldEnvError::Io(e) if e.kind() == std::io::ErrorKind::NotFound),
        "{err}"
    );
    assert!(err.to_string().starts_with("env file error: "));

    let f = env_file("HOST=a\nPORT='unterminated\n");
    let err = from_dotenv::<Settings>(f.path()).unwrap_err();
    assert!(matches!(err, VldEnvError::Source(_)), "{err}");

    let f = env_file("host=\nPORT=99999\nDEBUG=1\nTAGS=x\nDB__NAME=n\n");
    assert_eq!(
        var_issues(from_dotenv::<Settings>(f.path())),
        pairs(&[
            ("host", "too_small"),
            ("PORT", "too_big"),
            ("DEBUG", "invalid_type"),
            ("DB__POOL_SIZE", "missing_field"),
        ])
    );
}
