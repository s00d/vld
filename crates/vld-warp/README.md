# vld-warp

[Warp](https://docs.rs/warp) integration for the **vld** validation library.

## Features

| Filter | Source | Description |
|--------|--------|-------------|
| `vld_json::<T>()` | JSON body | Validates JSON request body |
| `vld_query::<T>()` | Query string | Validates URL query parameters |
| `handle_rejection` | — | Converts vld rejections into JSON responses |

Validation failures are returned as `422 Unprocessable Entity` with a JSON error body.

## Installation

```toml
[dependencies]
vld-warp = "0.1"
vld = "0.1"
warp = "0.3"
serde_json = "1"
```

## Quick Start

```rust
use vld_warp::prelude::*;
use warp::Filter;

vld::schema! {
    #[derive(Debug, Clone)]
    pub struct CreateUser {
        pub name: String  => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}

#[tokio::main]
async fn main() {
    let route = warp::post()
        .and(warp::path("users"))
        .and(vld_json::<CreateUser>())
        .map(|u: CreateUser| {
            warp::reply::json(&serde_json::json!({"name": u.name}))
        })
        .recover(handle_rejection);

    warp::serve(route).run(([0, 0, 0, 0], 3030)).await;
}
```

## Recovery Handler

Always add `.recover(handle_rejection)` to convert vld rejections into structured JSON:

```rust,ignore
let routes = create.or(search).recover(handle_rejection);
```

Error response format:

```json
{
  "error": "Validation failed",
  "issues": [
    { "path": ".name", "message": "String must be at least 2 characters" }
  ]
}
```

## Running Examples

```bash
cargo run -p vld-warp --example warp_basic
```

### Example Requests

```bash
# Create user (valid)
curl -X POST http://localhost:3030/users \
  -H 'Content-Type: application/json' \
  -d '{"name":"Alice","email":"alice@example.com","age":30}'

# Create user (invalid — triggers 422)
curl -X POST http://localhost:3030/users \
  -H 'Content-Type: application/json' \
  -d '{"name":"A","email":"bad","age":-1}'

# Search (query params)
curl "http://localhost:3030/search?q=hello&page=1&limit=10"
```

## License

MIT
