[![Crates.io](https://img.shields.io/crates/v/vld-rama?style=for-the-badge)](https://crates.io/crates/vld-rama)

# vld-rama

[Rama](https://ramaproxy.org/) integration for the [vld](https://crates.io/crates/vld) validation library.

Provides HTTP extractors and a `Layer` that validate request data with `vld` schemas — without requiring `serde::Deserialize`.

| API | Replaces | Source |
|---|---|---|
| `VldJson<T>` | `rama::http::service::web::extract::Json<T>` | JSON body |
| `VldQuery<T>` | `Query<T>` | URL query |
| `VldPath<T>` | `Path<T>` | Path params |
| `VldForm<T>` | `Form<T>` | `application/x-www-form-urlencoded` |
| `VldHeaders<T>` | manual headers | HTTP headers |
| `VldCookie<T>` | manual cookies | `Cookie` header |
| `ValidateJsonLayer<T>` | — | Middleware for Service stacks |

All extractors return **422 Unprocessable Entity** with a JSON issues body on failure.

## Installation

```toml
[dependencies]
vld = "0.5"
vld-rama = "0.5"
rama = { version = "0.4", features = ["http"] }
```

## Requires

- Rust **1.96+** (Rama's MSRV)
- `rama` with the `http` feature (pulled in by this crate)

## Quick example

```rust,ignore
use rama::http::service::web::WebService;
use vld::prelude::*;
use vld_rama::{VldJson, VldQuery};

vld::schema! {
    #[derive(Debug)]
    pub struct CreateUser {
        pub name: String => vld::string().min(2),
        pub email: String => vld::string().email(),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct Search {
        pub q: String => vld::string().min(1),
    }
}

async fn create(VldJson(user): VldJson<CreateUser>) -> String {
    format!("created {}", user.name)
}

async fn search(VldQuery(q): VldQuery<Search>) -> String {
    format!("q={}", q.q)
}

let svc = WebService::default()
    .with_post("/users", create)
    .with_get("/search", search);
```

## Layer (proxies / gateways)

When you compose a `Service` stack without extractors:

```rust,ignore
use vld_rama::{ValidateJsonLayer, validated, Validated};

// .layer(ValidateJsonLayer::<CreateUser>::new())   // or ::with_limit(bytes)
// then in a downstream service:
// let user: Option<CreateUser> = validated::<CreateUser>(&req);
```

Validated values are stored as `Validated<T>` in Rama `Extensions`. Non-JSON requests
(anything but `application/json` / `application/*+json`) are passed through untouched, so
`validated` returns `None` for them. JSON bodies over the limit (default 2 MiB) get `413`.

JSON body extractors require a JSON `Content-Type` (`application/json` or any `application/*+json`, case-insensitive); a missing or different type returns **415** `{"error":"Unsupported Media Type","message":"..."}`. Query, path, form, header and cookie values are coerced from text but follow the schema: `?q=123` into `vld::string()` stays `"123"`, and a single `?tags=a` fills a `Vec<String>`. Every issue in the 422 body carries a machine-readable `code` (e.g. `"too_small"`).

Form extractors require `application/x-www-form-urlencoded` (**415** otherwise) and accept at most 1000 fields (**422** beyond). Header names containing `_` are ignored (only `-` maps to `_`, so `x_user: admin` cannot fill an `x_user` field past a proxy that strips `X-User`), and every `Cookie` header is read — the first occurrence of a cookie name wins. `VldForm` reads only the body, never the query string. `VldPath` on a route without parameters sees an empty object.

## Errors and body limits

Every error uses the shared `vld` JSON body — 422 validation / malformed input, 413 payload too large, 415 wrong `Content-Type`; e.g. `{"error": "Validation failed", "issues": [{"path": ".name", "message": "...", "code": "too_small"}]}`. See [error bodies](https://github.com/s00d/vld/blob/main/crates/vld-http-common/README.md#error-bodies).

| Body | Default limit | Configure |
|---|---|---|
| JSON (`VldJson`) | 2 MiB | `VldBodyLimit(n)` request extension, e.g. `rama::layer::AddInputExtensionLayer::new(VldBodyLimit(n))` |
| Form (`VldForm`) | 2 MiB, max 1000 fields | `VldBodyLimit(n)` request extension, e.g. `rama::layer::AddInputExtensionLayer::new(VldBodyLimit(n))` |
| `ValidateJsonLayer` | 2 MiB | `ValidateJsonLayer::with_limit(n)` |

Limits of every adapter: [body limits](https://github.com/s00d/vld/blob/main/crates/vld-http-common/README.md#body-limits).

## Extractors vs Layer

- **Handlers / routers** → extractors (`VldJson`, …)
- **Proxy / gateway middleware** → `ValidateJsonLayer`

TCP / TLS / SOCKS / DNS do not need this crate — call `vld` schemas directly on your own payloads.

## License

MIT
