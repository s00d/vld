use serde::Serialize;
use vld_tauri::prelude::*;

// ---------------------------------------------------------------------------
// Schemas
// ---------------------------------------------------------------------------

vld::schema! {
    #[derive(Debug, Clone, Serialize)]
    pub struct CreateUser {
        pub name: String  => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
        pub age: Option<i64> => vld::number().int().min(0).max(150).optional(),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct Greeting {
        pub name: String => vld::string().min(1),
    }
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
    pub struct Progress {
        pub percent: i64 => vld::number().int().min(0).max(100),
        pub status: String => vld::string().min(1),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct UserUpdate {
        pub id: i64 => vld::number().int().min(1),
        pub name: String => vld::string().min(2),
    }
}

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct PluginConfig {
        pub api_key: String => vld::string().min(1),
        pub timeout: i64 => vld::number().int().min(100),
    }
}

// ===========================================================================
// validate()
// ===========================================================================

#[test]
fn validate_valid() {
    let val = serde_json::json!({"name": "Alice", "email": "alice@example.com"});
    let user = validate::<CreateUser>(val).unwrap();
    assert_eq!(user.name, "Alice");
    assert_eq!(user.email, "alice@example.com");
    assert_eq!(user.age, None);
}

#[test]
fn validate_with_optional() {
    let val = serde_json::json!({"name": "Bob", "email": "bob@test.com", "age": 25});
    let user = validate::<CreateUser>(val).unwrap();
    assert_eq!(user.age, Some(25));
}

#[test]
fn validate_invalid_returns_error() {
    let val = serde_json::json!({"name": "A", "email": "bad"});
    let err = validate::<CreateUser>(val).unwrap_err();
    assert_eq!(err.error, "Validation failed");
    assert!(err.issues.len() >= 2);
    for issue in &err.issues {
        assert!(!issue.message.is_empty());
    }
}

#[test]
fn validate_missing_fields() {
    let val = serde_json::json!({});
    let err = validate::<CreateUser>(val).unwrap_err();
    assert!(!err.issues.is_empty());
}

// ===========================================================================
// validate_args()
// ===========================================================================

#[test]
fn validate_args_valid() {
    let g = validate_args::<Greeting>(r#"{"name":"World"}"#).unwrap();
    assert_eq!(g.name, "World");
}

#[test]
fn validate_args_invalid_json() {
    let err = validate_args::<Greeting>("not json").unwrap_err();
    assert_eq!(err.error, "Invalid JSON");
    assert_eq!(err.issues.len(), 1);
}

#[test]
fn validate_args_invalid_data() {
    let err = validate_args::<Greeting>(r#"{"name":""}"#).unwrap_err();
    assert_eq!(err.error, "Validation failed");
}

// ===========================================================================
// validate_event()
// ===========================================================================

#[test]
fn event_valid() {
    let payload = serde_json::json!({"id": 42, "name": "Alice"});
    let update = validate_event::<UserUpdate>(payload).unwrap();
    assert_eq!(update.id, 42);
    assert_eq!(update.name, "Alice");
}

#[test]
fn event_invalid() {
    let payload = serde_json::json!({"id": 0, "name": "A"});
    let err = validate_event::<UserUpdate>(payload).unwrap_err();
    assert_eq!(err.error, "Validation failed");
    assert!(err.issue_count() >= 2);
}

// ===========================================================================
// validate_state()
// ===========================================================================

#[test]
fn state_valid() {
    let cfg = serde_json::json!({"db_url": "postgres://localhost/db", "max_conn": 10});
    let config = validate_state::<AppConfig>(cfg).unwrap();
    assert_eq!(config.max_conn, 10);
}

#[test]
fn state_invalid() {
    let cfg = serde_json::json!({"db_url": "", "max_conn": 0});
    let err = validate_state::<AppConfig>(cfg).unwrap_err();
    assert!(err.has_issues());
}

// ===========================================================================
// validate_plugin_config()
// ===========================================================================

#[test]
fn plugin_config_valid() {
    let cfg = serde_json::json!({"api_key": "abc123", "timeout": 5000});
    let c = validate_plugin_config::<PluginConfig>(cfg).unwrap();
    assert_eq!(c.api_key, "abc123");
    assert_eq!(c.timeout, 5000);
}

#[test]
fn plugin_config_invalid() {
    let cfg = serde_json::json!({"api_key": "", "timeout": 50});
    let err = validate_plugin_config::<PluginConfig>(cfg).unwrap_err();
    assert!(err.issue_count() >= 2);
}

// ===========================================================================
// validate_channel_message()
// ===========================================================================

#[test]
fn channel_message_valid() {
    let msg = serde_json::json!({"percent": 42, "status": "downloading"});
    let p = validate_channel_message::<Progress>(msg).unwrap();
    assert_eq!(p.percent, 42);
    assert_eq!(p.status, "downloading");
}

#[test]
fn channel_message_invalid() {
    let msg = serde_json::json!({"percent": 150, "status": ""});
    let err = validate_channel_message::<Progress>(msg).unwrap_err();
    assert!(err.has_issues());
}

// ===========================================================================
// VldTauriError
// ===========================================================================

#[test]
fn error_serializes_to_json() {
    let val = serde_json::json!({"name": "A", "email": "bad"});
    let err = validate::<CreateUser>(val).unwrap_err();
    let json = serde_json::to_value(&err).unwrap();
    assert_eq!(json["error"], "Validation failed");
    assert!(json["issues"].is_array());
    let issues = json["issues"].as_array().unwrap();
    assert!(!issues.is_empty());
    for issue in issues {
        assert!(issue["path"].is_string());
        assert!(issue["message"].is_string());
    }
}

#[test]
fn error_roundtrips() {
    let val = serde_json::json!({"name": "A", "email": "bad"});
    let err = validate::<CreateUser>(val).unwrap_err();
    let json_str = serde_json::to_string(&err).unwrap();
    let back: VldTauriError = serde_json::from_str(&json_str).unwrap();
    assert_eq!(back.error, "Validation failed");
    assert_eq!(back.issues.len(), err.issues.len());
}

#[test]
fn error_custom() {
    let err = VldTauriError::custom("My Error", "something went wrong");
    assert_eq!(err.error, "My Error");
    assert_eq!(err.issues.len(), 1);
    assert_eq!(err.issues[0].message, "something went wrong");
}

#[test]
fn error_json_parse() {
    let err = VldTauriError::json_parse_error("unexpected EOF");
    assert_eq!(err.error, "Invalid JSON");
    assert_eq!(err.issues[0].message, "unexpected EOF");
}

#[test]
fn error_display() {
    let val = serde_json::json!({"name": "A", "email": "bad"});
    let err = validate::<CreateUser>(val).unwrap_err();
    let display = format!("{err}");
    assert!(display.contains("Validation failed"));
    assert!(display.contains("issue(s)"));
}

// ===========================================================================
// VldPayload<T>
// ===========================================================================

#[test]
fn payload_deserialize_valid() {
    let val = serde_json::json!({"name": "Alice", "email": "alice@example.com"});
    let payload: VldPayload<CreateUser> = serde_json::from_value(val).unwrap();
    assert_eq!(payload.name, "Alice");
    assert_eq!(payload.email, "alice@example.com");
}

#[test]
fn payload_deserialize_invalid() {
    let val = serde_json::json!({"name": "A", "email": "bad"});
    let result: Result<VldPayload<CreateUser>, _> = serde_json::from_value(val);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Validation failed"));
}

#[test]
fn payload_deref_works() {
    let val = serde_json::json!({"name": "Bob", "email": "bob@test.com"});
    let payload: VldPayload<CreateUser> = serde_json::from_value(val).unwrap();
    let user: &CreateUser = &payload;
    assert_eq!(user.name, "Bob");
}

// ===========================================================================
// VldEvent<T>
// ===========================================================================

#[test]
fn vld_event_deserialize_valid() {
    let val = serde_json::json!({"id": 5, "name": "Carol"});
    let event: VldEvent<UserUpdate> = serde_json::from_value(val).unwrap();
    assert_eq!(event.id, 5);
    assert_eq!(event.name, "Carol");
}

#[test]
fn vld_event_deserialize_invalid() {
    let val = serde_json::json!({"id": 0, "name": "X"});
    let result: Result<VldEvent<UserUpdate>, _> = serde_json::from_value(val);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Validation failed"));
}

#[test]
fn vld_event_deref() {
    let val = serde_json::json!({"id": 10, "name": "Dave"});
    let event: VldEvent<UserUpdate> = serde_json::from_value(val).unwrap();
    let inner: &UserUpdate = &event;
    assert_eq!(inner.id, 10);
}
