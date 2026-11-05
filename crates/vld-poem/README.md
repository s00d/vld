[![Crates.io](https://img.shields.io/crates/v/vld-poem?style=for-the-badge)](https://crates.io/crates/vld-poem)
[![docs.rs](https://img.shields.io/docsrs/vld-poem?style=for-the-badge)](https://docs.rs/vld-poem)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=for-the-badge)](https://github.com/s00d/vld)
[![GitHub issues](https://img.shields.io/badge/github-issues-orange?style=for-the-badge)](https://github.com/s00d/vld/issues)
[![GitHub stars](https://img.shields.io/badge/github-stars-yellow?style=for-the-badge)](https://github.com/s00d/vld/stargazers)

# vld-poem

[Poem](https://docs.rs/poem) integration for the **vld** validation library.

## Features

| Extractor | Source | Description |
|-----------|--------|-------------|
| `VldJson<T>` | JSON body | Validates JSON request body |
| `VldQuery<T>` | Query string | Validates URL query parameters |
| `VldForm<T>` | Form body | Validates `application/x-www-form-urlencoded` |
| `VldPath<T>` | Path params | Validates path parameters |
| `VldHeaders<T>` | HTTP headers | Validates headers (`X-Request-Id` → `x_request_id`) |
| `VldCookie<T>` | Cookie header | Validates cookie values |

All extractors return `422 Unprocessable Entity` with a JSON error body on validation failure
or malformed input, and `413 Payload Too Large` (`{"error":"Payload too large"}`) for bodies over the
limit. Errors are `VldPoemError` (exported from the prelude; `status()` / `body()`).

JSON body extractors require a JSON `Content-Type` (`application/json` or any `application/*+json`, case-insensitive); a missing or different type returns **415** `{"error":"Unsupported Media Type","message":"..."}`. Query, path, form, header and cookie values are coerced from text but follow the schema: `?q=123` into `vld::string()` stays `"123"`, and a single `?tags=a` fills a `Vec<String>`. Every issue in the 422 body carries a machine-readable `code` (e.g. `"too_small"`).

Form extractors require `application/x-www-form-urlencoded` (**415** otherwise) and accept at most 1000 fields (**422** beyond). Header names containing `_` are ignored (only `-` maps to `_`, so `x_user: admin` cannot fill an `x_user` field past a proxy that strips `X-User`), and every `Cookie` header is read — the first occurrence of a cookie name wins.

Query/form/path/header/cookie values are coerced losslessly: canonical numbers → number,
`"true"`/`"false"` → boolean, anything else (`""`, `"null"`, `"01234"`) stays a string; repeated
keys and `key[]` become arrays.

## Installation

```toml
[dependencies]
vld-poem = "0.5"
vld = "0.5"
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

## Errors and body limits

Every error uses the shared `vld` JSON body — 422 validation / malformed input, 413 payload too large, 415 wrong `Content-Type`; e.g. `{"error": "Validation failed", "issues": [{"path": ".name", "message": "...", "code": "too_small"}]}`. See [error bodies](https://github.com/s00d/vld/blob/main/crates/vld-http-common/README.md#error-bodies).

| Body | Default limit | Configure |
|---|---|---|
| JSON (`VldJson`) | 2 MiB | `.data(VldBodyLimit(n))` on the endpoint / route |
| Form (`VldForm`) | 2 MiB, max 1000 fields | `.data(VldBodyLimit(n))` on the endpoint / route |

Limits of every adapter: [body limits](https://github.com/s00d/vld/blob/main/crates/vld-http-common/README.md#body-limits).

## OpenAPI (vld-utoipa)

Use [vld-utoipa](../vld-utoipa/README.md) with `#[into_params]` + `impl_to_schema!` for OpenAPI;
runtime validation via `VldJson` / `VldQuery` / `VldPath` `FromRequest` extractors.

## License

MIT
