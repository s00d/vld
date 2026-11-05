[![Crates.io](https://img.shields.io/crates/v/vld-axum?style=for-the-badge)](https://crates.io/crates/vld-axum)
[![docs.rs](https://img.shields.io/docsrs/vld-axum?style=for-the-badge)](https://docs.rs/vld-axum)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=for-the-badge)](https://github.com/s00d/vld)
[![GitHub issues](https://img.shields.io/badge/github-issues-orange?style=for-the-badge)](https://github.com/s00d/vld/issues)
[![GitHub stars](https://img.shields.io/badge/github-stars-yellow?style=for-the-badge)](https://github.com/s00d/vld/stargazers)

# vld-axum

[Axum](https://github.com/tokio-rs/axum) integration for the [vld](https://crates.io/crates/vld) validation library.

## Overview

Provides 6 extractors that validate request data using `vld` schemas **before** your handler runs:

| Extractor | Replaces | Source |
|---|---|---|
| `VldJson<T>` | `axum::Json<T>` | JSON request body |
| `VldQuery<T>` | `axum::extract::Query<T>` | URL query parameters |
| `VldPath<T>` | `axum::extract::Path<T>` | URL path parameters |
| `VldForm<T>` | `axum::extract::Form<T>` | URL-encoded form body |
| `VldHeaders<T>` | manual header extraction | HTTP headers |
| `VldCookie<T>` | manual cookie parsing | Cookie values |

On validation failure (and for malformed bodies) all extractors return **422 Unprocessable Entity** with a JSON body describing every issue; bodies over the size limit return **413** `{"error":"Payload too large"}`.

JSON body extractors require a JSON `Content-Type` (`application/json` or any `application/*+json`, case-insensitive); a missing or different type returns **415** `{"error":"Unsupported Media Type","message":"..."}`. Query, path, form, header and cookie values are coerced from text but follow the schema: `?q=123` into `vld::string()` stays `"123"`, and a single `?tags=a` fills a `Vec<String>`. Every issue in the 422 body carries a machine-readable `code` (e.g. `"too_small"`).

Form extractors require `application/x-www-form-urlencoded` (**415** otherwise) and accept at most 1000 fields (**422** beyond). Header names containing `_` are ignored (only `-` maps to `_`, so `x_user: admin` cannot fill an `x_user` field past a proxy that strips `X-User`), and every `Cookie` header is read — the first occurrence of a cookie name wins.

## Installation

```toml
[dependencies]
vld = "0.5"
vld-axum = "0.5"
axum = "0.8"
tokio = { version = "1", features = ["full"] }
```

## VldJson — JSON body

```rust
use axum::{Router, routing::post};
use vld_axum::VldJson;

vld::schema! {
    #[derive(Debug)]
    pub struct CreateUser {
        pub name: String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}

async fn create_user(VldJson(user): VldJson<CreateUser>) -> String {
    format!("Created: {}", user.name)
}
```

## VldQuery — query parameters

Values are coerced losslessly: canonical numbers (`"42"`, `"-1.5"`) → number, `"true"`/`"false"` → boolean; everything else (`""`, `"null"`, `"01234"`, `"+1555"`, `"1e3"`) stays a string. Repeated keys (`?tag=a&tag=b`) and `tag[]=a` become arrays. Use `vld::string().coerce()` for string fields that may look numeric.

```rust
use axum::{Router, routing::get};
use vld::prelude::*;
use vld_axum::VldQuery;

vld::schema! {
    #[derive(Debug)]
    pub struct SearchParams {
        pub q: String => vld::string().min(1).max(200),
        pub page: Option<i64> => vld::number().int().min(1).optional(),
        pub limit: Option<i64> => vld::number().int().min(1).max(100).optional(),
    }
}

async fn search(VldQuery(p): VldQuery<SearchParams>) -> String {
    format!("Searching '{}' page={:?}", p.q, p.page)
}
```

## VldPath — path parameters

```rust
use axum::{Router, routing::get};
use vld_axum::VldPath;

vld::schema! {
    #[derive(Debug)]
    pub struct UserPath {
        pub id: i64 => vld::number().int().min(1),
    }
}

async fn get_user(VldPath(p): VldPath<UserPath>) -> String {
    format!("User #{}", p.id)
}

let app = Router::new().route("/users/{id}", get(get_user));
```

## VldForm — URL-encoded form body

```rust
use axum::{Router, routing::post};
use vld_axum::VldForm;

vld::schema! {
    #[derive(Debug)]
    pub struct LoginForm {
        pub username: String => vld::string().min(3).max(50),
        pub password: String => vld::string().min(8),
    }
}

async fn login(VldForm(f): VldForm<LoginForm>) -> String {
    format!("Welcome, {}!", f.username)
}
```

## VldHeaders — HTTP headers

Header names are normalised to snake_case: `Content-Type` → `content_type`, `X-Request-Id` → `x_request_id`.

```rust
use axum::{Router, routing::get};
use vld::prelude::*;
use vld_axum::VldHeaders;

vld::schema! {
    #[derive(Debug)]
    pub struct AuthHeaders {
        pub authorization: String => vld::string().min(1),
        pub x_request_id: Option<String> => vld::string().optional(),
    }
}

async fn protected(VldHeaders(h): VldHeaders<AuthHeaders>) -> String {
    format!("auth={}", h.authorization)
}
```

## VldCookie — cookies

Cookie names are matched as-is to schema field names.

```rust
use axum::{Router, routing::get};
use vld::prelude::*;
use vld_axum::VldCookie;

vld::schema! {
    #[derive(Debug)]
    pub struct Session {
        pub session_id: String => vld::string().min(1),
        pub theme: Option<String> => vld::string().optional(),
    }
}

async fn dashboard(VldCookie(c): VldCookie<Session>) -> String {
    format!("session={}", c.session_id)
}
```

## Combining extractors

Multiple extractors can be used in the same handler:

```rust
use axum::{Router, routing::post};
use vld::prelude::*;
use vld_axum::{VldJson, VldQuery, VldHeaders};

vld::schema! {
    #[derive(Debug)]
    pub struct OrderQuery {
        pub dry_run: Option<bool> => vld::boolean().optional(),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct OrderBody {
        pub product_id: i64 => vld::number().int().min(1),
        pub quantity: i64 => vld::number().int().min(1).max(1000),
    }
}

async fn create_order(
    VldQuery(q): VldQuery<OrderQuery>,
    VldJson(b): VldJson<OrderBody>,
) -> String {
    format!("product={} qty={} dry_run={:?}", b.product_id, b.quantity, q.dry_run)
}
```

## Running the example

```sh
cargo run -p vld-axum --example axum_basic
```

### Example requests

```sh
# VldJson — create user:
curl -s -X POST http://localhost:3000/users \
  -H "Content-Type: application/json" \
  -d '{"name": "Alice", "email": "alice@example.com", "age": 25}' | jq

# VldPath — get user by id:
curl -s http://localhost:3000/users/42 | jq

# VldQuery — search:
curl -s "http://localhost:3000/search?q=rust&page=1&limit=20" | jq

# VldForm — login (URL-encoded):
curl -s -X POST http://localhost:3000/login \
  -d "username=alice&password=secret1234" | jq

# VldHeaders — protected endpoint:
curl -s http://localhost:3000/protected \
  -H "Authorization: Bearer mytoken123" \
  -H "X-Request-Id: abc-123" | jq

# VldCookie — dashboard:
curl -s http://localhost:3000/dashboard \
  -b "session_id=s3ss10n; theme=dark" | jq

# Combined — query + body:
curl -s -X POST "http://localhost:3000/orders?dry_run=true&currency=USD" \
  -H "Content-Type: application/json" \
  -d '{"product_id": 42, "quantity": 3}' | jq
```

## Errors and body limits

Every error uses the shared `vld` JSON body — 422 validation / malformed input, 413 payload too large, 415 wrong `Content-Type`; e.g. `{"error": "Validation failed", "issues": [{"path": ".name", "message": "...", "code": "too_small"}]}`. See [error bodies](https://github.com/s00d/vld/blob/main/crates/vld-http-common/README.md#error-bodies).

| Body | Default limit | Configure |
|---|---|---|
| JSON (`VldJson`) | 2 MiB | `.layer(axum::extract::DefaultBodyLimit::max(n))` |
| Form (`VldForm`) | 2 MiB, max 1000 fields | `.layer(axum::extract::DefaultBodyLimit::max(n))` |

Limits of every adapter: [body limits](https://github.com/s00d/vld/blob/main/crates/vld-http-common/README.md#body-limits).

## OpenAPI (vld-utoipa)

`VldJson` / `VldQuery` / `VldPath` validate at runtime. To emit the same vld rules in OpenAPI
(`minLength`, `minimum`, …) without duplicating `#[param(…)]`, use
[vld-utoipa](../vld-utoipa/README.md):

```toml
vld = { version = "0.5", features = ["openapi"] }
vld-utoipa = "0.5"
utoipa = "5"
```

```rust
use vld::prelude::*;
use vld_axum::{VldJson, VldQuery, VldPath};
use vld_utoipa::impl_to_schema;

vld::schema! {
    #[derive(Debug)]
    pub struct CreateUser {
        pub name: String => vld::string().min(2),
    }
}
impl_to_schema!(CreateUser);

vld::schema! {
    #[derive(Debug)]
    #[into_params(parameter_in = Query)]
    pub struct SearchParams {
        pub q: String => vld::string().min(1).max(200),
    }
}
impl_to_schema!(SearchParams);

vld::schema! {
    #[derive(Debug)]
    #[into_params(parameter_in = Path)]
    pub struct UserPath {
        pub id: i64 => vld::number().int().positive(),
    }
}
impl_to_schema!(UserPath);

#[utoipa::path(
    post,
    path = "/users/{id}/search",
    params(UserPath, SearchParams),
    request_body = CreateUser,
)]
async fn handler(
    VldPath(path): VldPath<UserPath>,
    VldQuery(query): VldQuery<SearchParams>,
    VldJson(body): VldJson<CreateUser>,
) { /* ... */ }
```

```bash
cargo run -p vld-axum --example axum_openapi
```

## License

MIT
