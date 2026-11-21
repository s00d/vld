# vld-openapi

[![crates.io](https://img.shields.io/crates/v/vld-openapi.svg)](https://crates.io/crates/vld-openapi)
[![docs.rs](https://img.shields.io/docsrs/vld-openapi)](https://docs.rs/vld-openapi)

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

## License

MIT
