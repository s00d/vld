# vld-http-common

[![crates.io](https://img.shields.io/crates/v/vld-http-common)](https://crates.io/crates/vld-http-common)
[![docs.rs](https://img.shields.io/docsrs/vld-http-common)](https://docs.rs/vld-http-common)

Shared HTTP helpers of the [vld](https://crates.io/crates/vld) web adapters (`vld-axum`, `vld-actix`, `vld-ntex`, `vld-poem`, `vld-rama`, `vld-rocket`, `vld-salvo`, `vld-tower`, `vld-warp`, `vld-tonic`): value coercion, query / form / header / cookie parsing, content-type checks and the JSON error body. Framework-agnostic — no HTTP framework or `http` dependency.

Applications normally use it through an adapter; depend on it directly to write an adapter for another framework.

## Install

```toml
[dependencies]
vld-http-common = "0.5"
```

| Feature | Description |
|---|---|
| `conformance` | Test support: the shared adapter conformance suite |

## Example

```rust
use vld_http_common::{parse_json_body, require_json_content_type, VldHttpError};

vld::schema! {
    #[derive(Debug)]
    pub struct User {
        pub name: String => vld::string().min(2),
    }
}

fn extract(content_type: Option<&str>, body: &[u8]) -> Result<User, VldHttpError> {
    require_json_content_type(content_type)?; // 415
    let value = parse_json_body(body)?;       // 422 on malformed JSON
    Ok(User::parse_value(&value)?)            // 422 on validation errors
}

let err = extract(Some("application/json"), br#"{"name": "A"}"#).unwrap_err();
assert_eq!(err.status(), 422);
// {"error": "Validation failed", "issues": [{"path": ".name", "message": "...", "code": "too_small"}]}
println!("{}", err.body());
```

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/shared/http-common>
- Shared adapter rules: <https://s00d.github.io/vld/integrations/web/overview>
- API: <https://docs.rs/vld-http-common>

MSRV: 1.85 ([MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md)).

## License

MIT
