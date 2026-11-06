# vld-tauri

[![crates.io](https://img.shields.io/crates/v/vld-tauri.svg)](https://crates.io/crates/vld-tauri)
[![docs.rs](https://docs.rs/vld-tauri/badge.svg)](https://docs.rs/vld-tauri)

[Tauri](https://tauri.app) IPC helpers for [vld](https://crates.io/crates/vld): validate command arguments, events, state and channel messages, and return a serializable error to the frontend. Framework-agnostic: no `tauri` dependency.

## Install

```toml
[dependencies]
vld = "0.5"
vld-tauri = "0.5"
```

## Example

```rust
use vld_tauri::prelude::*;

vld::schema! {
    #[derive(Debug, Clone)]
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

On failure the frontend receives `{"error": "Validation failed", "issues": [{"path": "name", "message": "..."}]}`.

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/ui/tauri>
- API: <https://docs.rs/vld-tauri>

MSRV: 1.85, see [MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md).

## License

MIT
