# vld-derive

[![crates.io](https://img.shields.io/crates/v/vld-derive.svg)](https://crates.io/crates/vld-derive)
[![docs.rs](https://img.shields.io/docsrs/vld-derive)](https://docs.rs/vld-derive)

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

## License

MIT
