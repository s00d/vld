# vld-dioxus

Dioxus integration for the [vld](https://crates.io/crates/vld) validation library.

**Define validation rules once — use them on both server and client (WASM).**

No direct dependency on `dioxus` — works with any Dioxus version (0.5, 0.6, 0.7+) and compiles for WASM targets.

## Installation

```toml
[dependencies]
vld = "0.3"
vld-dioxus = "0.3"
dioxus = "0.7"  # or your version
```

## Quick Start

### 1. Shared Validation Schemas

Define schema factories in a shared module compiled for both server and WASM:

```rust
// shared.rs
pub fn name_schema() -> vld::primitives::ZString {
    vld::string().min(2).max(50)
}

pub fn email_schema() -> vld::primitives::ZString {
    vld::string().email()
}

pub fn age_schema() -> vld::primitives::ZInt {
    vld::number().int().min(0).max(150)
}
```

### 2. Server Function Validation

Use `validate_args!` inside `#[server]` functions:

```rust
use dioxus::prelude::*;

#[server]
async fn create_user(
    name: String,
    email: String,
    age: i64,
) -> Result<(), ServerFnError> {
    vld_dioxus::validate_args! {
        name  => shared::name_schema(),
        email => shared::email_schema(),
        age   => shared::age_schema(),
    }
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    // ... insert into database
    Ok(())
}
```

### 3. Client-Side Reactive Validation

Use `check_field` inside Dioxus `use_memo` hooks for instant feedback:

```rust
#[component]
fn CreateUserForm() -> Element {
    let mut name = use_signal(String::new);
    let mut email = use_signal(String::new);

    let name_err = use_memo(move || {
        let v = name();
        if v.is_empty() { return None; }
        vld_dioxus::check_field(&v, &shared::name_schema())
    });

    let email_err = use_memo(move || {
        let v = email();
        if v.is_empty() { return None; }
        vld_dioxus::check_field(&v, &shared::email_schema())
    });

    rsx! {
        form {
            input {
                r#type: "text",
                placeholder: "Name",
                oninput: move |evt| name.set(evt.value()),
            }
            if let Some(err) = name_err() {
                span { class: "error", "{err}" }
            }

            input {
                r#type: "email",
                placeholder: "Email",
                oninput: move |evt| email.set(evt.value()),
            }
            if let Some(err) = email_err() {
                span { class: "error", "{err}" }
            }

            button { r#type: "submit", "Create" }
        }
    }
}
```

### 4. Server -> Client Error Display

Parse structured errors from server function responses:

```rust
#[component]
fn CreateUserForm() -> Element {
    let create = use_server_future(move || create_user(name(), email(), age()));

    let server_errors = use_memo(move || {
        create.value()
            .and_then(|r| r.as_ref().err())
            .and_then(|e| vld_dioxus::VldServerError::from_json(&e.to_string()))
    });

    rsx! {
        form {
            input { r#type: "text", name: "name" }
            if let Some(ref errs) = server_errors() {
                if errs.has_field_error("name") {
                    span { class: "error",
                        "{errs.field_error(\"name\").unwrap_or_default()}"
                    }
                }
            }
            // ...
        }
    }
}
```

## API Reference

### Error Types

| Type | Description |
|---|---|
| `VldServerError` | Structured error with per-field messages, serializable for transport |
| `FieldError` | Single field error: `{ field, message }` |

#### `VldServerError` Methods

| Method | Returns | Description |
|---|---|---|
| `validation(fields)` | `VldServerError` | Create from a list of `FieldError` |
| `internal(msg)` | `VldServerError` | Create an internal error |
| `field_error(name)` | `Option<&str>` | First error message for a field |
| `field_errors(name)` | `Vec<&str>` | All error messages for a field |
| `has_field_error(name)` | `bool` | Check if a field has errors |
| `error_fields()` | `Vec<&str>` | All field names with errors |
| `from_json(s)` | `Option<Self>` | Parse from JSON string |
| `to_string()` | `String` | Serialize to JSON (via `Display`) |

### Validation Functions

| Function | Use Case |
|---|---|
| `validate::<Schema, T>(data)` | Validate a `Serialize` struct against a `vld::schema!` type |
| `validate_value::<Schema>(json)` | Validate a `serde_json::Value` directly |
| `check_field(value, schema)` | Single-field check -> `Option<String>` error |
| `check_field_all(value, schema)` | Single-field check -> `Vec<String>` all errors |
| `check_all_fields::<Schema, T>(data)` | Multi-field check -> `Vec<FieldError>` |

### Macros

| Macro | Description |
|---|---|
| `validate_args! { field => schema, ... }` | Inline validation of server function arguments |

## Custom Error Type

For advanced use cases, wrap `VldServerError` in your own error enum:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppError {
    Validation(vld_dioxus::VldServerError),
    NotFound(String),
    Internal(String),
}

// Implement FromServerFnError for AppError...

#[server]
async fn create_user(name: String) -> Result<(), AppError> {
    vld_dioxus::validate_args! {
        name => vld::string().min(2),
    }
    .map_err(AppError::Validation)?;
    Ok(())
}
```

## Running the Example

```sh
cargo run -p vld-dioxus --example dioxus_basic
```

## License

MIT
