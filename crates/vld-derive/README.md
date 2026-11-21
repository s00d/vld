[![Crates.io](https://img.shields.io/crates/v/vld-derive?style=for-the-badge)](https://crates.io/crates/vld-derive)
[![Downloads](https://img.shields.io/crates/d/vld-derive?style=for-the-badge)](https://crates.io/crates/vld-derive)
[![docs.rs](https://img.shields.io/badge/docs-docs.rs-blue?style=for-the-badge)](https://docs.rs/vld-derive)
[![Guide](https://img.shields.io/badge/guide-s00d.github.io-blue?style=for-the-badge)](https://s00d.github.io/vld/structs/derive)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Donate](https://img.shields.io/badge/Donate-Donationalerts-ff4081?style=for-the-badge)](https://www.donationalerts.com/r/s00d88)

# vld-derive

`#[derive(Validate)]` for the [vld](https://crates.io/crates/vld) validation library.

Do not depend on this crate directly; enable `vld`'s `derive` feature, which re-exports the macro as `vld::Validate`.

## Install

```toml
[dependencies]
vld = { version = "0.8", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
```

## Example

```rust
use vld::Validate;

#[derive(Debug, serde::Serialize, Validate)]
#[serde(rename_all = "camelCase")]
struct User {
    #[vld(vld::string().min(2).max(50))]
    display_name: String,
    #[vld(vld::string().email())]
    email: String,
    #[vld(vld::number().int().gte(18).optional())]
    age: Option<i64>,
}

let user = User::vld_parse(r#"{"displayName": "Ada", "email": "ada@example.com"}"#).unwrap();
assert!(User::is_valid(&user));
```

The derive generates `vld_parse()`, `parse_value()`, `validate()`, `is_valid()`, `validate_field(name, value)` (one field by its deserialize name), the `VldParse` impl and, with `vld/openapi`, `json_schema()` / `to_openapi_document()`. `#[vld(lenient)]` on the struct adds `validate_fields()` and `parse_lenient()`. Enums are supported in serde's four representations.

A field without `#[vld(...)]` gets its type's default schema (`String`, `bool`, integers, floats, `Option<T>`, `Vec<T>`, `HashMap<String, T>`, `serde_json::Value`, derived types). Serde `rename`, `rename_all`, `skip`, `skip_deserializing`, `default` and `flatten` are honoured; `skip_serializing` / `skip_serializing_if` fields may be absent from the form `validate()` checks. Field `alias` is rejected at compile time.

## Documentation

- Guide: <https://s00d.github.io/vld/structs/derive>
- API: <https://docs.rs/vld-derive>

## Support

If vld helps you, you can support it via [DonationAlerts](https://www.donationalerts.com/r/s00d88).
Bugs and ideas: [issues](https://github.com/s00d/vld/issues).

## License

MIT
