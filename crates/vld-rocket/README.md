[![Crates.io](https://img.shields.io/crates/v/vld-rocket?style=for-the-badge)](https://crates.io/crates/vld-rocket)
[![docs.rs](https://img.shields.io/docsrs/vld-rocket?style=for-the-badge)](https://docs.rs/vld-rocket)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=for-the-badge)](https://github.com/s00d/vld)
[![GitHub issues](https://img.shields.io/badge/github-issues-orange?style=for-the-badge)](https://github.com/s00d/vld/issues)
[![GitHub stars](https://img.shields.io/badge/github-stars-yellow?style=for-the-badge)](https://github.com/s00d/vld/stargazers)

# vld-rocket

[Rocket](https://rocket.rs/) integration for the **vld** validation library.

## Features

| Extractor | Source | Description |
|-----------|--------|-------------|
| `VldJson<T>` | JSON body | Validates JSON request body |
| `VldQuery<T>` | Query string | Validates URL query parameters |
| `VldForm<T>` | Form body | Validates `application/x-www-form-urlencoded` |
| `VldPath<T>` | Path segments | Validates `<param>` / trailing `<param..>` segments (any mount point) |
| `VldHeaders<T>` | HTTP headers | Validates headers (`X-Request-Id` → `x_request_id`) |
| `VldCookie<T>` | Cookie header | Validates cookie values |

All extractors return `422 Unprocessable Entity` with a JSON error body on validation failure
or malformed input (invalid JSON, non-UTF-8 form), and `413 Payload Too Large` when the body
exceeds Rocket's `limits.json` / `limits.form` (`form` falls back to 2 MiB if unset).

Rocket requires every `<param>` to be a handler argument as well, so declare them next to
`VldPath<T>` (e.g. `fn get(id: &str, p: VldPath<UserPath>)`).

## Installation

```toml
[dependencies]
vld-rocket = "0.4"
vld = "0.4"
rocket = { version = "0.5", features = ["json"] }
serde_json = "1"
```

## Quick Start

```rust
use vld_rocket::prelude::*;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CreateUser {
        pub name: String  => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}

#[rocket::post("/users", data = "<user>")]
fn create_user(user: VldJson<CreateUser>) -> rocket::serde::json::Json<serde_json::Value> {
    rocket::serde::json::Json(serde_json::json!({
        "name": user.name,
        "email": user.email,
    }))
}

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    rocket::build()
        .mount("/", rocket::routes![create_user])
        .register("/", vld_rocket::vld_catchers())
        .launch()
        .await?;
    Ok(())
}
```

## Error Catchers

Register the built-in catchers to get JSON error responses:

```rust,ignore
.register("/", vld_rocket::vld_catchers()) // 400, 413, 415, 422

// or pick individually:
.register("/", rocket::catchers![
    vld_rocket::vld_422_catcher,  // validation / malformed body errors
    vld_rocket::vld_413_catcher,  // body over the configured limit
    vld_rocket::vld_415_catcher,
    vld_rocket::vld_400_catcher,
])
```

## Running Examples

```bash
cargo run -p vld-rocket --example rocket_basic
```

### Example Requests

```bash
# Create user (valid)
curl -X POST http://localhost:8000/users \
  -H 'Content-Type: application/json' \
  -d '{"name":"Alice","email":"alice@example.com","age":30}'

# Create user (invalid — triggers 422)
curl -X POST http://localhost:8000/users \
  -H 'Content-Type: application/json' \
  -d '{"name":"A","email":"bad","age":-1}'

# Search (query params)
curl "http://localhost:8000/search?q=hello&page=1&limit=10"
```

## OpenAPI (vld-utoipa)

Use [vld-utoipa](../vld-utoipa/README.md) with `#[into_params(parameter_in = Query)]` or `Path` and
`impl_to_schema!` for OpenAPI parameter constraints. Runtime: `VldJson<T>`, `VldQuery<T>`,
`VldPath<T>` in handler signatures.

## License

MIT
