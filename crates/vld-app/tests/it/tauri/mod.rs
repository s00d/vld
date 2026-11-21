//! `vld_app::tauri` behaviour tests.
//!
//! | File | Kind | What |
//! |---|---|---|
//! | `story_command.rs` | stories | An IPC command validates its payload (`validate`, `validate_args`); the frontend reads the serialized `VldTauriError`: paths, codes, params for built-in checks, rules, refinements, JSON parse and custom errors. Startup state / plugin config / channel messages. |
//! | `story_payload.rs` | stories | `VldPayload<T>` / `VldEvent<T>` deserialization: the value, `Deref`, the inline error string Tauri reports. |
//! | `model_error.rs` | model (proptest) | Random `VldError`s (paths, codes, messages) through `validate` and `VldPayload`: a model predicts every frontend issue (path, message, code, params) and the inline message. |
//!
//! Compared: values, issue paths / codes / params; message texts only where
//! the story is about the frontend's message.

mod common;

mod model_error;
mod story_command;
mod story_payload;
