# vld-poem

[Poem](https://docs.rs/poem) integration for the **vld** validation library.

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
vld-poem = "0.1"
vld = "0.1"
poem = "3"
serde_json = "1"
```

## Quick Start

```rust
use poem::{handler, listener::TcpListener, post, Route, Server};
use vld_poem::prelude::*;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CreateUser {
        pub name: String  => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}

#[handler]
async fn create_user(user: VldJson<CreateUser>) -> poem::web::Json<serde_json::Value> {
    poem::web::Json(serde_json::json!({
        "name": user.name,
        "email": user.email,
    }))
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let app = Route::new().at("/users", post(create_user));
    Server::new(TcpListener::bind("0.0.0.0:3000"))
        .run(app)
        .await
}
```

## Running Examples

```bash
cargo run -p vld-poem --example poem_basic
```

### Example Requests

```bash
# Create user (valid)
curl -X POST http://localhost:3000/users \
  -H 'Content-Type: application/json' \
  -d '{"name":"Alice","email":"alice@example.com","age":30}'

# Create user (invalid — triggers 422)
curl -X POST http://localhost:3000/users \
  -H 'Content-Type: application/json' \
  -d '{"name":"A","email":"bad","age":-1}'

# Search (query params)
curl "http://localhost:3000/search?q=hello&page=1&limit=10"
```

## License

MIT
