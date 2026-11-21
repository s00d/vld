//! Stories: a Tauri command validates its payload and the frontend reads the
//! serialized error.

use super::common::*;

use serde_json::json;
use vld::rules::Rule;
use vld_app::tauri::prelude::*;

vld::schema! {
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct CreateUser {
        pub name: String  => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
        pub age: Option<i64> => vld::number().int().min(0).max(150).optional(),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Address {
        pub city: String => vld::string().min(1),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Order {
        pub address: Address => vld::nested(Address::parse_value),
        pub items: Vec<String> => vld::array(vld::string().min(2)),
    }
}

/// The command body as an app writes it.
fn create_user(payload: serde_json::Value) -> Result<String, VldTauriError> {
    let user = validate::<CreateUser>(payload)?;
    Ok(format!("Created {}", user.name))
}

/// Given a `create_user` command validating `CreateUser`,
/// when the frontend sends valid payloads (with and without the optional age,
/// at the inclusive bounds),
/// then the command gets the typed value.
#[test]
fn valid_payload_reaches_the_command_typed() {
    let user =
        validate::<CreateUser>(json!({"name": "Alice", "email": "alice@example.com"})).unwrap();
    assert_eq!(
        (user.name.as_str(), user.email.as_str(), user.age),
        ("Alice", "alice@example.com", None)
    );

    let user =
        validate::<CreateUser>(json!({"name": "Bob", "email": "bob@test.com", "age": 25})).unwrap();
    assert_eq!(user.age, Some(25));

    let edge = json!({"name": "Al", "email": "a@b.co", "age": 150});
    assert_eq!(validate::<CreateUser>(edge).unwrap().age, Some(150));

    assert_eq!(
        create_user(json!({"name": "Carol", "email": "c@d.io"})).unwrap(),
        "Created Carol"
    );
}

/// Given the `create_user` command,
/// when the frontend sends a payload breaking three checks,
/// then the rejected promise carries `{error, issues}` with one issue per
/// field: the path without a leading dot, the code and the params an i18n
/// library fills its template with; the error round-trips through JSON and an
/// older payload without `code` / `params` still deserializes.
#[test]
fn frontend_renders_issues_from_code_and_params() {
    let err = create_user(json!({"name": "A", "email": "bad", "age": 200})).unwrap_err();

    // What the frontend receives.
    let wire = serde_json::to_value(&err).unwrap();
    assert_eq!(wire["error"], "Validation failed");
    let issues = wire["issues"].as_array().unwrap();
    assert_eq!(issues.len(), 3);
    for i in issues {
        let keys: Vec<&str> = i.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["code", "message", "params", "path"]);
        assert!(!i["message"].as_str().unwrap().is_empty());
    }

    assert_eq!(
        paths_codes(&err),
        [
            ("name", "too_small"),
            ("email", "invalid_string"),
            ("age", "too_big")
        ]
    );
    assert_eq!(
        issue(&err, "name").params,
        params(&[
            ("inclusive", "true"),
            ("minimum", "2"),
            ("origin", "string")
        ])
    );
    assert_eq!(
        issue(&err, "email").params,
        params(&[("validation", "email")])
    );
    assert_eq!(
        issue(&err, "age").params,
        params(&[
            ("inclusive", "true"),
            ("maximum", "150"),
            ("origin", "number")
        ])
    );

    assert!(err.has_issues());
    assert_eq!(err.issue_count(), 3);
    assert_eq!(err.to_string(), "Validation failed: 3 issue(s)");
    let as_std: Box<dyn std::error::Error> = Box::new(err.clone());
    assert_eq!(as_std.to_string(), "Validation failed: 3 issue(s)");

    let back: VldTauriError = serde_json::from_str(&serde_json::to_string(&err).unwrap()).unwrap();
    assert_eq!(back.error, err.error);
    assert_eq!(back.issues, err.issues);

    let old: VldTauriError = serde_json::from_value(json!({
        "error": "Validation failed",
        "issues": [{"path": "name", "message": "Too short"}]
    }))
    .unwrap();
    assert_eq!(
        old.issues,
        [TauriIssue {
            path: "name".into(),
            message: "Too short".into(),
            code: String::new(),
            params: params(&[]),
        }]
    );
}

/// Given commands with flat, nested and array payloads,
/// when fields are missing, mistyped or invalid deep inside,
/// then every issue names its field as the frontend addresses it
/// (`address.city`, `items[1]`) with the code of what went wrong.
#[test]
fn missing_mistyped_and_nested_fields() {
    let err = validate::<CreateUser>(json!({})).unwrap_err();
    assert_eq!(
        paths_codes(&err),
        [("name", "missing_field"), ("email", "missing_field")]
    );
    assert_eq!(issue(&err, "name").params, params(&[]));

    let err = validate::<CreateUser>(json!({"name": 5, "email": "a@b.co"})).unwrap_err();
    assert_eq!(paths_codes(&err), [("name", "invalid_type")]);
    assert_eq!(
        issue(&err, "name").params,
        params(&[("expected", "string"), ("received", "number")])
    );

    let err = validate::<Order>(json!({"address": {"city": ""}, "items": ["ok", "x", "fine"]}))
        .unwrap_err();
    assert_eq!(
        paths_codes(&err),
        [("address.city", "too_small"), ("items[1]", "too_small")]
    );

    // A payload that is not an object at all: one issue at the root.
    let err = validate::<CreateUser>(json!(42)).unwrap_err();
    assert_eq!(paths_codes(&err), [("", "invalid_type")]);
    assert_eq!(issue(&err, "").params["expected"], "object");
}

/// Russian mobile number: `+7` and ten digits.
struct RuPhone;

impl Rule<str> for RuPhone {
    fn key(&self) -> &str {
        "ru_phone"
    }
    fn check(&self, s: &str) -> bool {
        s.strip_prefix("+7")
            .is_some_and(|d| d.len() == 10 && d.bytes().all(|b| b.is_ascii_digit()))
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Profile {
        pub phone: String => vld::string().rule(RuPhone),
        pub lucky: i64 => vld::number().int().rule(vld::rules::from_fn("even", |n: &i64| n % 2 == 0)),
        pub nick: String => vld::string().refine(|s: &String| !s.contains(' '), "No spaces"),
        pub code: String => vld::string().super_refine(|s: &String, errors: &mut vld::error::VldError| {
            if s.len() < 3 {
                errors.push(IssueCode::Custom { code: "code_short".into() }, "Too short");
            }
            if !s.starts_with('X') {
                errors.push(IssueCode::Custom { code: "code_prefix".into() }, "Must start with X");
            }
        }),
    }
}

/// Given a command whose fields use reusable rules, a refinement and a
/// multi-error refinement,
/// when every custom check fails,
/// then each issue's code is the rule's key (`custom` for `refine`) and its
/// params carry that key, so the frontend can translate rules by key.
#[test]
fn custom_rules_report_their_key() {
    let ok = json!({"phone": "+79991234567", "lucky": 4, "nick": "neo", "code": "XYZ"});
    validate::<Profile>(ok).unwrap();

    let err = validate::<Profile>(json!({"phone": "8999", "lucky": 3, "nick": "a b", "code": "y"}))
        .unwrap_err();
    assert_eq!(
        paths_codes(&err),
        [
            ("phone", "ru_phone"),
            ("lucky", "even"),
            ("nick", "custom"),
            ("code", "code_short"),
            ("code", "code_prefix"),
        ]
    );
    for i in &err.issues {
        assert_eq!(i.params, params(&[("code", &i.code)]), "{i:?}");
    }
    // The refinement's own message reaches the frontend unchanged.
    assert_eq!(issue(&err, "nick").message, "No spaces");
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Greeting {
        pub name: String => vld::string().min(1),
    }
}

/// Given a command taking the raw JSON argument string,
/// when the string is valid, is not JSON, or is JSON failing the schema,
/// then the command gets the value, an `Invalid JSON` error with one
/// `invalid_json` issue carrying the parser's message, or the validation
/// issues; an app's own failure goes out as a `custom` issue.
#[test]
fn raw_json_arguments_and_app_errors() {
    assert_eq!(
        validate_args::<Greeting>(r#"{"name":"World"}"#)
            .unwrap()
            .name,
        "World"
    );

    let err = validate_args::<Greeting>("not json").unwrap_err();
    let parser_msg = serde_json::from_str::<serde_json::Value>("not json")
        .unwrap_err()
        .to_string();
    assert_eq!(err.error, "Invalid JSON");
    assert_eq!(
        err.issues,
        [TauriIssue {
            path: String::new(),
            message: parser_msg,
            code: "invalid_json".into(),
            params: params(&[]),
        }]
    );

    let err = validate_args::<Greeting>(r#"{"name":""}"#).unwrap_err();
    assert_eq!(err.error, "Validation failed");
    assert_eq!(paths_codes(&err), [("name", "too_small")]);

    let err = VldTauriError::json_parse_error("unexpected EOF");
    assert_eq!((err.error.as_str(), err.issue_count()), ("Invalid JSON", 1));
    assert_eq!(err.issues[0].message, "unexpected EOF");

    let err = VldTauriError::custom("Database", "user exists");
    assert_eq!(err.error, "Database");
    assert_eq!(
        err.issues,
        [TauriIssue {
            path: String::new(),
            message: "user exists".into(),
            code: "custom".into(),
            params: params(&[]),
        }]
    );
    assert_eq!(err.to_string(), "Database: 1 issue(s)");
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct AppConfig {
        pub db_url: String => vld::string().min(1),
        pub max_conn: i64 => vld::number().int().min(1).max(100),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct PluginConfig {
        pub api_key: String => vld::string().min(1),
        pub timeout: i64 => vld::number().int().min(100),
    }
}

vld::schema! {
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct Progress {
        pub percent: i64 => vld::number().int().min(0).max(100),
        pub status: String => vld::string().min(1),
    }
}

/// Given an app that validates its state at startup, a plugin's config from
/// `tauri.conf.json` and the messages it streams over a channel,
/// when each is valid or broken,
/// then the valid ones come back typed and the broken ones list every bad key.
#[test]
fn startup_state_plugin_config_and_channel_messages() {
    let cfg = validate::<AppConfig>(json!({"db_url": "postgres://localhost/db", "max_conn": 10}))
        .unwrap();
    assert_eq!(
        (cfg.db_url.as_str(), cfg.max_conn),
        ("postgres://localhost/db", 10)
    );
    let err = validate::<AppConfig>(json!({"db_url": "", "max_conn": 0})).unwrap_err();
    assert_eq!(
        paths_codes(&err),
        [("db_url", "too_small"), ("max_conn", "too_small")]
    );

    let p = validate::<PluginConfig>(json!({"api_key": "abc123", "timeout": 5000})).unwrap();
    assert_eq!((p.api_key.as_str(), p.timeout), ("abc123", 5000));
    let err = validate::<PluginConfig>(json!({"api_key": "", "timeout": 50})).unwrap_err();
    assert_eq!(
        paths_codes(&err),
        [("api_key", "too_small"), ("timeout", "too_small")]
    );

    // Channel: the backend validates what it is about to send.
    let msg = Progress {
        percent: 42,
        status: "downloading".into(),
    };
    let sent = validate::<Progress>(serde_json::to_value(&msg).unwrap()).unwrap();
    assert_eq!((sent.percent, sent.status.as_str()), (42, "downloading"));
    let err = validate::<Progress>(json!({"percent": 150, "status": ""})).unwrap_err();
    assert_eq!(
        paths_codes(&err),
        [("percent", "too_big"), ("status", "too_small")]
    );
}
