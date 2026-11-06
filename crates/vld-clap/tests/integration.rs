use vld::Validate;
use vld_clap::prelude::*;
use vld_clap::ErrorSource;

// ---------------------------------------------------------------------------
// Derive Validate directly on the CLI struct — no separate schema needed
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Serialize, Validate)]
struct ServerCli {
    #[vld(vld::string().min(1))]
    host: String,
    #[vld(vld::number().int().min(1).max(65535))]
    port: i64,
    #[vld(vld::number().int().min(1).max(256))]
    workers: i64,
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

// ---------------------------------------------------------------------------
// Tests: validate — struct with #[derive(Validate)]
// ---------------------------------------------------------------------------

/// `(path, code)` of every validation issue (panics on `Ok` / serialization errors).
#[track_caller]
fn issues<T: std::fmt::Debug>(r: Result<T, VldClapError>) -> Vec<(String, String)> {
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

/// Owned `(path, code)` pairs for comparing with [`issues`].
fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
    expected
        .iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect()
}

#[test]
fn server_valid() {
    let cli = ServerCli {
        host: "0.0.0.0".into(),
        port: 8080,
        workers: 4,
    };
    validate(&cli).unwrap();
    // Inclusive bounds.
    let edge = ServerCli {
        host: "h".into(),
        port: 65535,
        workers: 256,
    };
    validate(&edge).unwrap();
}

#[test]
fn server_invalid_port() {
    let cli = ServerCli {
        host: "localhost".into(),
        port: 99999,
        workers: 4,
    };
    let err = validate(&cli).unwrap_err();
    assert!(err.message.contains("--port"), "msg: {}", err.message);
}

#[test]
fn server_invalid_workers() {
    let cli = ServerCli {
        host: "localhost".into(),
        port: 3000,
        workers: 0,
    };
    let err = validate(&cli).unwrap_err();
    assert!(err.message.contains("--workers"), "msg: {}", err.message);
}

#[test]
fn server_empty_host() {
    let cli = ServerCli {
        host: "".into(),
        port: 3000,
        workers: 2,
    };
    let err = validate(&cli).unwrap_err();
    assert!(err.message.contains("--host"), "msg: {}", err.message);
}

#[test]
fn user_valid() {
    let cli = UserCli {
        email: "alice@example.com".into(),
        name: "Alice".into(),
        age: 30,
    };
    validate(&cli).unwrap();
    let bad = UserCli { age: 151, ..cli };
    assert_eq!(issues(validate(&bad)), pairs(&[(".age", "too_big")]));
}

#[test]
fn user_invalid_email() {
    let cli = UserCli {
        email: "not-an-email".into(),
        name: "Alice".into(),
        age: 30,
    };
    let err = validate(&cli).unwrap_err();
    assert!(err.message.contains("email"), "msg: {}", err.message);
}

#[test]
fn user_multiple_errors() {
    let cli = UserCli {
        email: "bad".into(),
        name: "X".into(),
        age: -5,
    };
    let err = validate(&cli).unwrap_err();
    if let vld_clap::ErrorSource::Validation(ref vld_err) = err.source {
        assert!(vld_err.issues.len() >= 3, "issues: {:?}", vld_err.issues);
    } else {
        panic!("Expected Validation error");
    }
}

// ---------------------------------------------------------------------------
// Tests: validate_with_schema (old pattern, separate schema)
// ---------------------------------------------------------------------------

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct PortSchema {
        pub port: i64 => vld::number().int().min(1).max(65535),
    }
}

#[test]
fn with_schema_valid() {
    #[derive(serde::Serialize)]
    struct Args {
        port: i64,
    }
    let args = Args { port: 3000 };
    let parsed = validate_with_schema::<PortSchema, _>(&args).unwrap();
    assert_eq!(parsed.port, 3000);
}

#[test]
fn with_schema_invalid() {
    #[derive(serde::Serialize)]
    struct Args {
        port: i64,
    }
    let args = Args { port: 0 };
    let result = validate_with_schema::<PortSchema, _>(&args);
    assert_eq!(issues(result), pairs(&[(".port", "too_small")]));
}

// ---------------------------------------------------------------------------
// Tests: validate_json
// ---------------------------------------------------------------------------

#[test]
fn json_valid() {
    let json = serde_json::json!({"host": "127.0.0.1", "port": 3000, "workers": 8});
    let cli = validate_json::<ServerCli>(&json).unwrap();
    assert_eq!(
        (cli.host.as_str(), cli.port, cli.workers),
        ("127.0.0.1", 3000, 8)
    );
}

#[test]
fn json_invalid() {
    let json = serde_json::json!({"host": "", "port": 0, "workers": 999});
    let result = validate_json::<ServerCli>(&json);
    assert_eq!(
        issues(result),
        pairs(&[
            (".host", "too_small"),
            (".port", "too_small"),
            (".workers", "too_big")
        ])
    );
}

// ---------------------------------------------------------------------------
// Tests: error formatting
// ---------------------------------------------------------------------------

#[test]
fn error_format_issues() {
    let cli = ServerCli {
        host: "".into(),
        port: 0,
        workers: 0,
    };
    let err = validate(&cli).unwrap_err();
    let formatted = err.format_issues();
    assert!(formatted.contains("--host"), "formatted: {}", formatted);
    assert!(formatted.contains("--port"), "formatted: {}", formatted);
    assert!(formatted.contains("--workers"), "formatted: {}", formatted);
}

#[test]
fn error_is_std_error() {
    let cli = ServerCli {
        host: "ok".into(),
        port: -1,
        workers: 2,
    };
    let err = validate(&cli).unwrap_err();
    let dyn_err: &dyn std::error::Error = &err;
    let source = dyn_err.source().expect("wraps the VldError");
    assert!(source.downcast_ref::<vld::error::VldError>().is_some());
    assert!(dyn_err.to_string().contains("--port"), "{dyn_err}");
}

#[derive(Debug, serde::Serialize, Validate)]
struct SnakeCli {
    #[vld(vld::number().int().min(1))]
    max_connections: i64,
}

#[test]
fn flag_names_are_kebab_case_without_double_dot() {
    let err = validate(&SnakeCli { max_connections: 0 }).unwrap_err();
    assert!(
        err.message.contains("--max-connections:"),
        "msg: {}",
        err.message
    );
    assert!(err.format_issues().contains("--max-connections:"));
    assert!(!err.message.contains("--."), "msg: {}", err.message);
}
