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

All extractors return `422 Unprocessable Entity` with a JSON error body on validation failure.

## Installation

```toml
[dependencies]
vld-rocket = "0.1"
vld = "0.1"
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
        .register("/", rocket::catchers![
            vld_rocket::vld_422_catcher,
            vld_rocket::vld_400_catcher,
        ])
        .launch()
        .await?;
    Ok(())
}
```

## Error Catchers

Register the built-in catchers to get JSON error responses:

```rust,ignore
.register("/", rocket::catchers![
    vld_rocket::vld_422_catcher,  // validation errors
    vld_rocket::vld_400_catcher,  // JSON parse errors
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

## License

MIT
