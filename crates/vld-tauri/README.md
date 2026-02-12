[![Crates.io](https://img.shields.io/crates/v/vld-tauri?style=for-the-badge)](https://crates.io/crates/vld-tauri)
[![docs.rs](https://img.shields.io/docsrs/vld-tauri?style=for-the-badge)](https://docs.rs/vld-tauri)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=for-the-badge)](https://github.com/s00d/vld)
[![GitHub issues](https://img.shields.io/badge/github-issues-orange?style=for-the-badge)](https://github.com/s00d/vld/issues)
[![GitHub stars](https://img.shields.io/badge/github-stars-yellow?style=for-the-badge)](https://github.com/s00d/vld/stargazers)

# vld-tauri

[Tauri](https://tauri.app/) validation for the **vld** validation library.

**Zero dependency on `tauri` itself** — only `vld` + `serde` + `serde_json`.
Add `tauri` separately in your app.

## What can be validated

| Area | Function / Type | Description |
|------|-----------------|-------------|
| IPC commands | `validate()`, `VldPayload<T>` | Validate `#[tauri::command]` arguments |
| Events | `validate_event()`, `VldEvent<T>` | Validate `emit()`/`listen()` payloads |
| App state | `validate_state()` | Validate config/state before `app.manage()` |
| Plugin config | `validate_plugin_config()` | Validate plugin JSON configuration |
| Channels | `validate_channel_message()` | Validate outgoing `Channel::send()` data |
| Raw JSON | `validate_args()` | Validate a raw JSON string |
| Error type | `VldTauriError` | Serializable error for command results |

## Installation

```toml
[dependencies]
vld-tauri = "0.1"
vld = "0.1"
tauri = "2"
serde_json = "1"
```

## IPC Commands

### Pattern 1 — Explicit Validation (recommended)

```rust,ignore
use vld_tauri::prelude::*;

vld::schema! {
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct CreateUser {
        pub name: String  => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}

#[tauri::command]
fn create_user(payload: serde_json::Value) -> Result<String, VldTauriError> {
    let user = validate::<CreateUser>(payload)?;
    Ok(format!("Created {}", user.name))
}
```

### Pattern 2 — Auto-Validated Payload

`VldPayload<T>` validates during deserialization. Fields accessible via `Deref`.

```rust,ignore
#[tauri::command]
fn create_user(payload: VldPayload<CreateUser>) -> Result<String, VldTauriError> {
    Ok(format!("Created {}", payload.name))
}
```

## Event Payloads

```rust,ignore
use tauri::Listener;

// Explicit validation
app.listen("user:update", |event| {
    let payload: serde_json::Value = serde_json::from_str(event.payload()).unwrap();
    match validate_event::<UserUpdate>(payload) {
        Ok(update) => println!("Valid: {:?}", update),
        Err(e) => eprintln!("Invalid event: {e}"),
    }
});

// Auto-validation via VldEvent<T>
app.listen("user:update", |event| {
    match serde_json::from_str::<VldEvent<UserUpdate>>(event.payload()) {
        Ok(update) => println!("id={}", update.id),
        Err(e) => eprintln!("Bad payload: {e}"),
    }
});
```

## State Validation at Init

```rust,ignore
let config_json = std::fs::read_to_string("config.json")?;
let config = validate_state::<AppConfig>(
    serde_json::from_str(&config_json)?
).expect("Invalid config");
app.manage(config);
```

## Plugin Config Validation

```rust,ignore
let plugin_cfg: serde_json::Value = /* from tauri.conf.json */;
let cfg = validate_plugin_config::<MyPluginConfig>(plugin_cfg)
    .expect("Invalid plugin config");
```

## Channel Messages

```rust,ignore
#[tauri::command]
fn stream(channel: tauri::ipc::Channel<serde_json::Value>) -> Result<(), VldTauriError> {
    let msg = serde_json::json!({"percent": 50, "status": "working"});
    let validated = validate_channel_message::<Progress>(msg)?;
    channel.send(serde_json::to_value(&validated).unwrap()).unwrap();
    Ok(())
}
```

## Error Format

`VldTauriError` implements `Serialize`, so Tauri returns it directly:

```json
{
  "error": "Validation failed",
  "issues": [
    { "path": ".name", "message": "String must be at least 2 characters" },
    { "path": ".email", "message": "Invalid email address" }
  ]
}
```

## Frontend Usage (TypeScript)

```typescript
import { invoke } from '@tauri-apps/api/core';

interface VldError {
  error: string;
  issues: Array<{ path: string; message: string }>;
}

try {
  const result = await invoke('create_user', {
    payload: { name: 'Alice', email: 'alice@example.com' }
  });
} catch (err) {
  const vldErr = err as VldError;
  for (const issue of vldErr.issues) {
    console.error(`${issue.path}: ${issue.message}`);
  }
}
```

## License

MIT
