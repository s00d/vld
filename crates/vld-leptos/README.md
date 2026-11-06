# vld-leptos

[![crates.io](https://img.shields.io/crates/v/vld-leptos.svg)](https://crates.io/crates/vld-leptos)
[![docs.rs](https://docs.rs/vld-leptos/badge.svg)](https://docs.rs/vld-leptos)

[Leptos](https://leptos.dev) helpers for [vld](https://crates.io/crates/vld): one set of schemas for server-function arguments and client-side form fields. Framework-agnostic: no `leptos` dependency; builds for native and WASM.

## Install

```toml
[dependencies]
vld = "0.5"
vld-leptos = "0.5"
```

## Example

```rust
fn name_schema() -> vld::primitives::ZString { vld::string().min(2).max(50) }
fn email_schema() -> vld::primitives::ZString { vld::string().email() }

#[server]
async fn create_user(name: String, email: String) -> Result<(), ServerFnError> {
    vld_leptos::validate_args! {
        name  => name_schema(),
        email => email_schema(),
    }
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

// In a component:
let name_error = Memo::new(move |_| vld_leptos::check_field(&name.get(), &name_schema()));
```

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/ui/leptos>
- API: <https://docs.rs/vld-leptos>

MSRV: 1.85, see [MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md).

## License

MIT
