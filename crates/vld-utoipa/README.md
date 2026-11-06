# vld-utoipa

[![crates.io](https://img.shields.io/crates/v/vld-utoipa.svg)](https://crates.io/crates/vld-utoipa)
[![docs.rs](https://img.shields.io/docsrs/vld-utoipa)](https://docs.rs/vld-utoipa)

Use [vld](https://crates.io/crates/vld) schemas as [utoipa](https://crates.io/crates/utoipa) `ToSchema` / `IntoParams` types: validation rules and OpenAPI docs from one definition.

## Install

```toml
[dependencies]
vld = { version = "0.5", features = ["openapi"] }
vld-utoipa = "0.5"
utoipa = "5"
```

## Features

| Feature | Default | Description |
|---|---|---|
| `utoipa-5` | yes | utoipa 5.x |
| `utoipa-6` | no | utoipa 6.x (`default-features = false, features = ["utoipa-6"]`) |

Enable exactly one.

## Example

```rust
use vld::prelude::*;
use vld_utoipa::impl_to_schema;

vld::schema! {
    #[derive(Debug)]
    pub struct CreateUser {
        pub name: String => vld::string().min(2).max(100),
        pub email: String => vld::string().email(),
    }
}
impl_to_schema!(CreateUser);

vld::schema! {
    #[derive(Debug)]
    #[into_params(parameter_in = Query)]
    pub struct SearchQuery {
        pub q: String => vld::string().min(1).max(200),
    }
}
impl_to_schema!(SearchQuery);

#[utoipa::path(post, path = "/users", request_body = CreateUser, params(SearchQuery))]
fn create_user() {}
```

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/openapi/utoipa>
- API: <https://docs.rs/vld-utoipa>

MSRV: 1.85 (`utoipa-6`: 1.88), see [MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md).

## License

MIT
