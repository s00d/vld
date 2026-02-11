use vld::Validate;
use vld_clap::prelude::*;

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

#[test]
fn server_valid() {
    let cli = ServerCli {
        host: "0.0.0.0".into(),
        port: 8080,
        workers: 4,
    };
    assert!(validate(&cli).is_ok());
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
    assert!(validate(&cli).is_ok());
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
    let result = validate_with_schema::<PortSchema, _>(&args);
    assert!(result.is_ok());
}

#[test]
fn with_schema_invalid() {
    #[derive(serde::Serialize)]
    struct Args {
        port: i64,
    }
    let args = Args { port: 0 };
    let result = validate_with_schema::<PortSchema, _>(&args);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Tests: validate_json
// ---------------------------------------------------------------------------

#[test]
fn json_valid() {
    let json = serde_json::json!({"host": "127.0.0.1", "port": 3000, "workers": 8});
    let result = validate_json::<ServerCli>(&json);
    assert!(result.is_ok());
}

#[test]
fn json_invalid() {
    let json = serde_json::json!({"host": "", "port": 0, "workers": 999});
    let result = validate_json::<ServerCli>(&json);
    assert!(result.is_err());
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
    let _: &dyn std::error::Error = &err;
}
