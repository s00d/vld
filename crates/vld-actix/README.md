[![Crates.io](https://img.shields.io/crates/v/vld-actix?style=for-the-badge)](https://crates.io/crates/vld-actix)
[![docs.rs](https://img.shields.io/docsrs/vld-actix?style=for-the-badge)](https://docs.rs/vld-actix)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=for-the-badge)](https://github.com/s00d/vld)
[![GitHub issues](https://img.shields.io/badge/github-issues-orange?style=for-the-badge)](https://github.com/s00d/vld/issues)
[![GitHub stars](https://img.shields.io/badge/github-stars-yellow?style=for-the-badge)](https://github.com/s00d/vld/stargazers)

# vld-actix

[Actix-web](https://actix.rs/) integration for the [vld](https://crates.io/crates/vld) validation library.

## Overview

Provides 6 extractors that validate request data using `vld` schemas **before** your handler runs:

| Extractor | Replaces | Source |
|---|---|---|
| `VldJson<T>` | `actix_web::web::Json<T>` | JSON request body |
| `VldQuery<T>` | `actix_web::web::Query<T>` | URL query parameters |
| `VldPath<T>` | `actix_web::web::Path<T>` | URL path parameters |
| `VldForm<T>` | `actix_web::web::Form<T>` | URL-encoded form body |
| `VldHeaders<T>` | manual header extraction | HTTP headers |
| `VldCookie<T>` | manual cookie parsing | Cookie values |

On validation failure all extractors return **422 Unprocessable Entity** with a JSON body describing every issue.

## Installation

```toml
[dependencies]
vld = "0.1"
vld-actix = "0.1"
actix-web = "4"
```

## VldJson — JSON body

```rust
use actix_web::{web, HttpResponse};
use vld_actix::VldJson;

vld::schema! {
    #[derive(Debug)]
    pub struct CreateUser {
        pub name: String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
    }
}

async fn create_user(body: VldJson<CreateUser>) -> HttpResponse {
    HttpResponse::Ok().body(format!("Created: {}", body.name))
}
```

## VldQuery — query parameters

Values are automatically coerced: `"42"` → number, `"true"`/`"false"` → boolean, empty → null.

```rust
use actix_web::HttpResponse;
use vld::prelude::*;
use vld_actix::VldQuery;

vld::schema! {
    #[derive(Debug)]
    pub struct SearchParams {
        pub q: String => vld::string().min(1).max(200),
        pub page: Option<i64> => vld::number().int().min(1).optional(),
        pub limit: Option<i64> => vld::number().int().min(1).max(100).optional(),
    }
}

async fn search(params: VldQuery<SearchParams>) -> HttpResponse {
    HttpResponse::Ok().body(format!("Searching '{}' page={:?}", params.q, params.page))
}
```

## VldPath — path parameters

```rust
use actix_web::{web, HttpResponse};
use vld_actix::VldPath;

vld::schema! {
    #[derive(Debug)]
    pub struct UserPath {
        pub id: i64 => vld::number().int().min(1),
    }
}

async fn get_user(path: VldPath<UserPath>) -> HttpResponse {
    HttpResponse::Ok().body(format!("User #{}", path.id))
}

// Register: .route("/users/{id}", web::get().to(get_user))
```

## VldForm — URL-encoded form body

```rust
use actix_web::HttpResponse;
use vld_actix::VldForm;

vld::schema! {
    #[derive(Debug)]
    pub struct LoginForm {
        pub username: String => vld::string().min(3).max(50),
        pub password: String => vld::string().min(8),
    }
}

async fn login(form: VldForm<LoginForm>) -> HttpResponse {
    HttpResponse::Ok().body(format!("Welcome, {}!", form.username))
}
```

## VldHeaders — HTTP headers

Header names are normalised to snake_case: `Content-Type` → `content_type`, `X-Request-Id` → `x_request_id`.

```rust
use actix_web::HttpResponse;
use vld::prelude::*;
use vld_actix::VldHeaders;

vld::schema! {
    #[derive(Debug)]
    pub struct AuthHeaders {
        pub authorization: String => vld::string().min(1),
        pub x_request_id: Option<String> => vld::string().optional(),
    }
}

async fn protected(headers: VldHeaders<AuthHeaders>) -> HttpResponse {
    HttpResponse::Ok().body(format!("auth={}", headers.authorization))
}
```

## VldCookie — cookies

Cookie names are matched as-is to schema field names.

```rust
use actix_web::HttpResponse;
use vld::prelude::*;
use vld_actix::VldCookie;

vld::schema! {
    #[derive(Debug)]
    pub struct Session {
        pub session_id: String => vld::string().min(1),
        pub theme: Option<String> => vld::string().optional(),
    }
}

async fn dashboard(cookies: VldCookie<Session>) -> HttpResponse {
    HttpResponse::Ok().body(format!("session={}", cookies.session_id))
}
```

## Combining extractors

Multiple extractors can be used in the same handler:

```rust
use actix_web::HttpResponse;
use vld::prelude::*;
use vld_actix::{VldJson, VldQuery};

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
    query: VldQuery<OrderQuery>,
    body: VldJson<OrderBody>,
) -> HttpResponse {
    HttpResponse::Ok().body(format!(
        "product={} qty={} dry_run={:?}",
        body.product_id, body.quantity, query.dry_run,
    ))
}
```

## Running the example

```sh
cargo run -p vld-actix --example actix_basic
```

### Example requests

```sh
# VldJson — create user:
curl -s -X POST http://localhost:8080/users \
  -H "Content-Type: application/json" \
  -d '{"name": "Alice", "email": "alice@example.com"}' | jq

# VldPath — get user by id:
curl -s http://localhost:8080/users/42 | jq

# VldQuery — search:
curl -s "http://localhost:8080/search?q=rust&page=1&limit=20" | jq

# VldForm — login (URL-encoded):
curl -s -X POST http://localhost:8080/login \
  -d "username=alice&password=secret1234" | jq

# VldHeaders — protected endpoint:
curl -s http://localhost:8080/protected \
  -H "Authorization: Bearer mytoken123" \
  -H "X-Request-Id: abc-123" | jq

# VldCookie — dashboard:
curl -s http://localhost:8080/dashboard \
  -b "session_id=s3ss10n; theme=dark" | jq

# Combined — query + body:
curl -s -X POST "http://localhost:8080/orders?dry_run=true&currency=USD" \
  -H "Content-Type: application/json" \
  -d '{"product_id": 42, "quantity": 3}' | jq
```

## Error response format

```json
{
  "error": "Validation failed",
  "issues": [
    { "path": "name", "message": "String must contain at least 2 character(s)", "code": "too_small" }
  ]
}
```

## License

MIT
