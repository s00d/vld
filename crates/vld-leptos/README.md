# vld-leptos

Leptos integration for the [vld](https://crates.io/crates/vld) validation library.

**Define validation rules once — use them on both server and client (WASM).**

No direct dependency on `leptos` — works with any Leptos version (0.6, 0.7, 0.8+) and compiles for WASM targets.

## Installation

```toml
[dependencies]
vld = "0.1"
vld-leptos = "0.1"
leptos = "0.7"  # or your version
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
use leptos::prelude::*;

#[server]
async fn create_user(
    name: String,
    email: String,
    age: i64,
) -> Result<(), ServerFnError> {
    vld_leptos::validate_args! {
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

Use `check_field` inside Leptos memos for instant feedback:

```rust
#[component]
fn CreateUserForm() -> impl IntoView {
    let (name, set_name) = signal(String::new());
    let (email, set_email) = signal(String::new());

    let name_err = Memo::new(move |_| {
        let v = name.get();
        if v.is_empty() { return None; } // don't validate empty
        vld_leptos::check_field(&v, &shared::name_schema())
    });

    let email_err = Memo::new(move |_| {
        let v = email.get();
        if v.is_empty() { return None; }
        vld_leptos::check_field(&v, &shared::email_schema())
    });

    view! {
        <form>
            <input
                type="text"
                placeholder="Name"
                on:input=move |ev| set_name.set(event_target_value(&ev))
            />
            <Show when=move || name_err.get().is_some()>
                <span class="error">{move || name_err.get().unwrap_or_default()}</span>
            </Show>

            <input
                type="email"
                placeholder="Email"
                on:input=move |ev| set_email.set(event_target_value(&ev))
            />
            <Show when=move || email_err.get().is_some()>
                <span class="error">{move || email_err.get().unwrap_or_default()}</span>
            </Show>

            <button type="submit">"Create"</button>
        </form>
    }
}
```

### 4. Server → Client Error Display

Parse structured errors returned from server functions:

```rust
#[component]
fn CreateUserForm() -> impl IntoView {
    let action = ServerAction::<CreateUser>::new();

    let server_errors = Memo::new(move |_| {
        action.value().get().and_then(|result| {
            result.err().and_then(|e| {
                vld_leptos::VldServerError::from_json(&e.to_string())
            })
        })
    });

    view! {
        <ActionForm action>
            <input type="text" name="name" />
            <Show when=move || {
                server_errors.get()
                    .map(|e| e.has_field_error("name"))
                    .unwrap_or(false)
            }>
                <span class="error">
                    {move || server_errors.get()
                        .and_then(|e| e.field_error("name").map(String::from))
                        .unwrap_or_default()}
                </span>
            </Show>

            <input type="email" name="email" />
            <button type="submit">"Create"</button>
        </ActionForm>
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
| `check_field(value, schema)` | Single-field check → `Option<String>` error |
| `check_field_all(value, schema)` | Single-field check → `Vec<String>` all errors |
| `check_all_fields::<Schema, T>(data)` | Multi-field check → `Vec<FieldError>` |

### Macros

| Macro | Description |
|---|---|
| `validate_args! { field => schema, ... }` | Inline validation of server function arguments |

## Custom Error Type

For advanced use cases, wrap `VldServerError` in your own error enum:

```rust
use serde::{Deserialize, Serialize};
use server_fn::codec::JsonEncoding;
use leptos::server_fn::error::{FromServerFnError, ServerFnErrorErr};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppError {
    Validation(vld_leptos::VldServerError),
    NotFound(String),
    Internal(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Validation(e) => write!(f, "{}", e),
            AppError::NotFound(msg) => write!(f, "Not found: {}", msg),
            AppError::Internal(msg) => write!(f, "Internal: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

impl FromServerFnError for AppError {
    type Encoder = JsonEncoding;
    fn from_server_fn_error(value: ServerFnErrorErr) -> Self {
        AppError::Internal(value.to_string())
    }
}

#[server]
async fn create_user(name: String) -> Result<(), AppError> {
    vld_leptos::validate_args! {
        name => vld::string().min(2),
    }
    .map_err(AppError::Validation)?;
    Ok(())
}
```

## Schema-Based Alternative

Instead of individual schema functions, use a `vld::schema!` struct:

```rust
vld::schema! {
    struct CreateUserSchema {
        name: String => vld::string().min(2).max(50),
        email: String => vld::string().email(),
    }
}

#[derive(Serialize)]
struct FormData { name: String, email: String }

// Server: validate full struct
let data = FormData { name, email };
vld_leptos::validate::<CreateUserSchema, _>(&data)?;

// Client: check all fields at once
let errors = vld_leptos::check_all_fields::<CreateUserSchema, _>(&data);
```

## Running the Example

```sh
cargo run -p vld-leptos --example leptos_basic
```

## License

MIT
