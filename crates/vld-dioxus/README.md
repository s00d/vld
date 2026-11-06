# vld-dioxus

[![crates.io](https://img.shields.io/crates/v/vld-dioxus.svg)](https://crates.io/crates/vld-dioxus)
[![docs.rs](https://docs.rs/vld-dioxus/badge.svg)](https://docs.rs/vld-dioxus)

[Dioxus](https://dioxuslabs.com) helpers for [vld](https://crates.io/crates/vld): one set of schemas for server-function arguments and client-side form fields. Framework-agnostic: no `dioxus` dependency; builds for native and WASM.

## Install

```toml
[dependencies]
vld = "0.5"
vld-dioxus = "0.5"
```

## Example

```rust
fn name_schema() -> vld::primitives::ZString { vld::string().min(2).max(50) }
fn email_schema() -> vld::primitives::ZString { vld::string().email() }

#[server]
async fn create_user(name: String, email: String) -> Result<(), ServerFnError> {
    vld_dioxus::validate_args! {
        name  => name_schema(),
        email => email_schema(),
    }
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

// In a component:
let name_error = use_memo(move || vld_dioxus::check_field(&name(), &name_schema()));
```

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/ui/dioxus>
- API: <https://docs.rs/vld-dioxus>

MSRV: 1.85, see [MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md).

## License

MIT
