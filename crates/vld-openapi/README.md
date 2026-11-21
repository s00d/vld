[![Crates.io](https://img.shields.io/crates/v/vld-openapi?style=for-the-badge)](https://crates.io/crates/vld-openapi)
[![Downloads](https://img.shields.io/crates/d/vld-openapi?style=for-the-badge)](https://crates.io/crates/vld-openapi)
[![docs.rs](https://img.shields.io/badge/docs-docs.rs-blue?style=for-the-badge)](https://docs.rs/vld-openapi)
[![Guide](https://img.shields.io/badge/guide-s00d.github.io-blue?style=for-the-badge)](https://s00d.github.io/vld/integrations/openapi/utoipa)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Donate](https://img.shields.io/badge/Donate-Donationalerts-ff4081?style=for-the-badge)](https://www.donationalerts.com/r/s00d88)

# vld-openapi

Use [vld](https://crates.io/crates/vld) schemas in utoipa, schemars and aide, so validation rules and OpenAPI docs come from one definition.

```toml
[dependencies]
vld = "0.8"
vld-openapi = { version = "0.8", features = ["utoipa-5"] }
utoipa = "5"
```

```rust
vld::schema! {
    #[derive(Debug)]
    pub struct CreateUser {
        pub name: String => vld::string().min(2).max(100),
        pub email: String => vld::string().email(),
    }
}
vld_openapi::utoipa5::impl_to_schema!(CreateUser); // utoipa ToSchema + IntoParams

#[derive(utoipa::OpenApi)]
#[openapi(components(schemas(CreateUser)))]
struct ApiDoc; // properties carry minLength, maxLength, format: email
```

| Module | Feature | What it does |
|---|---|---|
| `utoipa5` / `utoipa6` | `utoipa-5` / `utoipa-6` | `impl_to_schema!`: utoipa `ToSchema` / `IntoParams` from a vld type |
| `schemars0` / `schemars1` | `schemars-0` / `schemars-1` | `impl_json_schema!` (vld → `schemars::JsonSchema`, also for aide 0.15 / 0.16) and `impl_vld_parse!` (schemars type → vld validation) |
| `validator` | always | validate JSON against a JSON Schema, inspect and combine schemas, `to_openapi3` |

No feature is on by default; any set can be enabled together.

- Guide: <https://s00d.github.io/vld/integrations/openapi/utoipa>, <https://s00d.github.io/vld/integrations/openapi/schemars>
- API: <https://docs.rs/vld-openapi>

## Support

If vld helps you, you can support it via [DonationAlerts](https://www.donationalerts.com/r/s00d88).
Bugs and ideas: [issues](https://github.com/s00d/vld/issues).

## License

MIT
