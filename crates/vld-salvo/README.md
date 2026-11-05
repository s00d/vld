[![Crates.io](https://img.shields.io/crates/v/vld-salvo?style=for-the-badge)](https://crates.io/crates/vld-salvo)
[![docs.rs](https://img.shields.io/docsrs/vld-salvo?style=for-the-badge)](https://docs.rs/vld-salvo)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=for-the-badge)](https://github.com/s00d/vld)
[![GitHub issues](https://img.shields.io/badge/github-issues-orange?style=for-the-badge)](https://github.com/s00d/vld/issues)
[![GitHub stars](https://img.shields.io/badge/github-stars-yellow?style=for-the-badge)](https://github.com/s00d/vld/stargazers)

# vld-salvo

[Salvo](https://salvo.rs) integration for the **vld** validation library.

## Compatibility

Supports Salvo **1.x** (`salvo = "1"`). Crate MSRV is **1.94** (required by Salvo 1.0).

## Features

All extractors implement Salvo's `Extractible` trait and can be used
**directly as `#[handler]` parameters** — just like Salvo's built-in
`JsonBody` or `PathParam`:

| Extractor | Source | Description |
|-----------|--------|-------------|
| `VldJson<T>` | JSON body | Validates JSON request body |
| `VldQuery<T>` | Query string | Validates URL query parameters |
| `VldForm<T>` | Form body | Validates URL-encoded form body |
| `VldPath<T>` | Path params | Validates path parameters |
| `VldHeaders<T>` | HTTP headers | Validates request headers |
| `VldCookie<T>` | Cookie header | Validates cookie values |

All extractors implement `Deref<Target = T>` so you access fields
directly (e.g. `body.name` instead of `body.0.name`).

`VldSalvoError` implements Salvo's `Writer` trait — validation
failures render as `422 Unprocessable Entity` with a JSON error body (`413` above the
body limit, `415` for a wrong `Content-Type`); it exposes `error()`, `status()` and `body()`.

JSON body extractors require a JSON `Content-Type` (`application/json` or any `application/*+json`, case-insensitive); a missing or different type returns **415** `{"error":"Unsupported Media Type","message":"..."}`. Query, path, form, header and cookie values are coerced from text but follow the schema: `?q=123` into `vld::string()` stays `"123"`, and a single `?tags=a` fills a `Vec<String>`. Every issue in the 422 body carries a machine-readable `code` (e.g. `"too_small"`).

Form extractors require `application/x-www-form-urlencoded` (**415** otherwise) and accept at most 1000 fields (**422** beyond). Header names containing `_` are ignored (only `-` maps to `_`, so `x_user: admin` cannot fill an `x_user` field past a proxy that strips `X-User`), and every `Cookie` header is read — the first occurrence of a cookie name wins.

## Installation

```toml
[dependencies]
vld-salvo = "0.5"
vld = "0.5"
salvo = "1"
serde_json = "1"
```

## Quick Start

```rust
use salvo::prelude::*;
use vld_salvo::prelude::*;

vld::schema! {
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct CreateUser {
        pub name: String  => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}

// VldJson<T> is used directly as a handler parameter!
#[handler]
async fn create(body: VldJson<CreateUser>, res: &mut Response) {
    // Deref lets you access fields directly
    res.render(Json(serde_json::json!({"name": body.name})));
}

#[tokio::main]
async fn main() {
    let router = Router::with_path("users").post(create);
    let acceptor = TcpListener::new("0.0.0.0:5800").bind().await;
    Server::new(acceptor).serve(router).await;
}
```

## Path Parameters

Salvo uses `{name}` syntax for path parameters:

```rust,ignore
vld::schema! {
    #[derive(Debug, Clone)]
    pub struct UserId {
        pub id: i64 => vld::number().int().min(1),
    }
}

#[handler]
async fn get_user(p: VldPath<UserId>, res: &mut Response) {
    res.render(Json(serde_json::json!({"id": p.id})));
}

// Router::with_path("users/{id}").get(get_user)
```

## Multiple Extractors

Combine several extractors in a single handler:

```rust,ignore
#[handler]
async fn handler(
    path: VldPath<UserId>,
    query: VldQuery<Pagination>,
    headers: VldHeaders<AuthHeaders>,
    body: VldJson<UpdateUser>,
    res: &mut Response,
) {
    // path.id, query.page, headers.authorization, body.name
}
```

## Errors and body limits

Every error uses the shared `vld` JSON body — 422 validation / malformed input, 413 payload too large, 415 wrong `Content-Type`; e.g. `{"error": "Validation failed", "issues": [{"path": ".name", "message": "...", "code": "too_small"}]}`. See [error bodies](https://github.com/s00d/vld/blob/main/crates/vld-http-common/README.md#error-bodies).

| Body | Default limit | Configure |
|---|---|---|
| JSON (`VldJson`) | 2 MiB | `SecureMaxSize` middleware, `Request::set_secure_max_size` or `set_global_secure_max_size` |
| Form (`VldForm`) | 2 MiB, max 1000 fields | `SecureMaxSize` middleware, `Request::set_secure_max_size` or `set_global_secure_max_size` |

Salvo's built-in 64 KiB `secure_max_size` counts as unset (the shared 2 MiB applies), so setting exactly 64 KiB does too.

Limits of every adapter: [body limits](https://github.com/s00d/vld/blob/main/crates/vld-http-common/README.md#body-limits).

## Running Examples

```bash
cargo run -p vld-salvo --example salvo_basic
```

### Example Requests

```bash
# Create user (valid)
curl -X POST http://localhost:5800/users \
  -H 'Content-Type: application/json' \
  -d '{"name":"Alice","email":"alice@example.com","age":30}'

# Create user (invalid — triggers 422)
curl -X POST http://localhost:5800/users \
  -H 'Content-Type: application/json' \
  -d '{"name":"A","email":"bad"}'

# Get user by id
curl http://localhost:5800/users/42

# Search (query params)
curl "http://localhost:5800/search?q=hello&page=1&limit=10"

# Health check
curl http://localhost:5800/health
```

## OpenAPI (vld-utoipa)

Use [vld-utoipa](../vld-utoipa/README.md): `#[into_params(parameter_in = Query)]` +
`impl_to_schema!(SearchParams)` for `params(SearchParams)` in utoipa. Salvo handlers use
`body: VldJson<T>`, `q: VldQuery<T>`, `p: VldPath<T>` for validation.

## License

MIT
