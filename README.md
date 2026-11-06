# vld

[![crates.io](https://img.shields.io/crates/v/vld.svg)](https://crates.io/crates/vld)
[![docs.rs](https://img.shields.io/docsrs/vld)](https://docs.rs/vld)
[![CI](https://github.com/s00d/vld/actions/workflows/ci.yml/badge.svg)](https://github.com/s00d/vld/actions/workflows/ci.yml)
[![docs](https://img.shields.io/badge/docs-s00d.github.io%2Fvld-orange)](https://s00d.github.io/vld/)
[![license: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/s00d/vld/blob/main/LICENSE)

**Type-safe runtime validation for Rust, inspired by [Zod](https://zod.dev).**

Describe the shape of your data once, parse untrusted input into typed Rust values, and get every problem back with its path and a stable error code. The same schemas produce JSON Schema / OpenAPI and plug into web frameworks, databases, queues, config and CLI crates.

**Documentation: [s00d.github.io/vld](https://s00d.github.io/vld/)** · API reference: [docs.rs/vld](https://docs.rs/vld)

```toml
[dependencies]
vld = "0.5"
```

```rust
use vld::prelude::*;

vld::schema! {
    #[derive(Debug)]
    pub struct User {
        pub name: String => vld::string().trim().min(2).max(50),
        pub email: String => vld::string().email(),
        pub age: Option<u8> => vld::u8().min(18).optional(),
        pub tags: Vec<String> => vld::array(vld::string()).max_len(5).with_default(vec![]),
    }
}

fn main() {
    let user = User::parse(r#"{"name": " Ada ", "email": "ada@example.com"}"#).unwrap();
    assert_eq!(user.name, "Ada");

    let err = User::parse(r#"{"name": "A", "email": "nope", "age": 12}"#).unwrap_err();
    assert_eq!(err.issues.len(), 3); // every issue, not just the first
    println!("{err}");
    // .name: String must be at least 2 characters, received "A"
    // .email: Invalid email address, received "nope"
    // .age: Number must be at least 18, received 12
}
```

## Features

- **Schemas as values**: strings with 40+ formats, exact sized integers, floats, dates (chrono, jiff or time), collections, tuples, unions, discriminated unions, refinements, transforms, recursive schemas.
- **Typed structs** with `vld::schema!` or `#[derive(Validate)]` (feature `derive`), serde renames, lenient parsing.
- **Errors you can use**: typed `IssueCode`s with stable keys, paths, received values, custom messages, `flatten` / `treeify` / `prettify`, bundled English, Russian, German and Spanish messages.
- **JSON Schema and OpenAPI 3.1** from the same schemas (feature `openapi`), plus breaking-change detection (feature `diff`).
- **Small core**: only `serde` and `serde_json` by default; heavier validators are opt-in features.
- **Integration crates** for 10 web frameworks, OpenAPI tools, databases, queues, config, CLI and UI, with one error format and safe defaults (2 MiB body limits, 415 / 422 handling).

## Crates

| Crate | Purpose | Docs |
|---|---|---|
| [`vld`](https://crates.io/crates/vld) | Core: schemas, macros, errors, i18n, JSON Schema | [guide](https://s00d.github.io/vld/getting-started/introduction) · [api](https://docs.rs/vld) |
| [`vld-derive`](https://crates.io/crates/vld-derive) | `#[derive(Validate)]` (use through `vld`'s `derive` feature) | [guide](https://s00d.github.io/vld/structs/derive) · [api](https://docs.rs/vld-derive) |
| **Web** | | |
| [`vld-axum`](https://crates.io/crates/vld-axum) | Axum extractors | [guide](https://s00d.github.io/vld/integrations/web/axum) · [api](https://docs.rs/vld-axum) |
| [`vld-actix`](https://crates.io/crates/vld-actix) | Actix Web extractors | [guide](https://s00d.github.io/vld/integrations/web/actix) · [api](https://docs.rs/vld-actix) |
| [`vld-ntex`](https://crates.io/crates/vld-ntex) | ntex 3 / 4 extractors | [guide](https://s00d.github.io/vld/integrations/web/ntex) · [api](https://docs.rs/vld-ntex) |
| [`vld-poem`](https://crates.io/crates/vld-poem) | Poem extractors | [guide](https://s00d.github.io/vld/integrations/web/poem) · [api](https://docs.rs/vld-poem) |
| [`vld-rama`](https://crates.io/crates/vld-rama) | Rama extractors and validation layer | [guide](https://s00d.github.io/vld/integrations/web/rama) · [api](https://docs.rs/vld-rama) |
| [`vld-rocket`](https://crates.io/crates/vld-rocket) | Rocket data guards and JSON catchers | [guide](https://s00d.github.io/vld/integrations/web/rocket) · [api](https://docs.rs/vld-rocket) |
| [`vld-salvo`](https://crates.io/crates/vld-salvo) | Salvo extractors | [guide](https://s00d.github.io/vld/integrations/web/salvo) · [api](https://docs.rs/vld-salvo) |
| [`vld-tower`](https://crates.io/crates/vld-tower) | Tower layer for any Tower-based server | [guide](https://s00d.github.io/vld/integrations/web/tower) · [api](https://docs.rs/vld-tower) |
| [`vld-warp`](https://crates.io/crates/vld-warp) | Warp filters | [guide](https://s00d.github.io/vld/integrations/web/warp) · [api](https://docs.rs/vld-warp) |
| [`vld-tonic`](https://crates.io/crates/vld-tonic) | tonic gRPC messages and metadata | [guide](https://s00d.github.io/vld/integrations/web/tonic) · [api](https://docs.rs/vld-tonic) |
| **OpenAPI and JSON Schema** | | |
| [`vld-utoipa`](https://crates.io/crates/vld-utoipa) | utoipa `ToSchema` / `IntoParams` from vld schemas | [guide](https://s00d.github.io/vld/integrations/openapi/utoipa) · [api](https://docs.rs/vld-utoipa) |
| [`vld-schemars`](https://crates.io/crates/vld-schemars) | Two-way bridge with schemars | [guide](https://s00d.github.io/vld/integrations/openapi/schemars) · [api](https://docs.rs/vld-schemars) |
| [`vld-aide`](https://crates.io/crates/vld-aide) | aide (OpenAPI for axum) | [guide](https://s00d.github.io/vld/integrations/openapi/aide) · [api](https://docs.rs/vld-aide) |
| **Codegen and test data** | | |
| [`vld-ts`](https://crates.io/crates/vld-ts) | Zod / Valibot TypeScript from vld schemas | [guide](https://s00d.github.io/vld/integrations/codegen/ts) · [api](https://docs.rs/vld-ts) |
| [`vld-fake`](https://crates.io/crates/vld-fake) | Fake data that satisfies a schema | [guide](https://s00d.github.io/vld/integrations/codegen/fake) · [api](https://docs.rs/vld-fake) |
| **Databases** | | |
| [`vld-sqlx`](https://crates.io/crates/vld-sqlx) | SQLx column wrappers and pre-write checks | [guide](https://s00d.github.io/vld/integrations/database/sqlx) · [api](https://docs.rs/vld-sqlx) |
| [`vld-diesel`](https://crates.io/crates/vld-diesel) | Diesel column wrappers | [guide](https://s00d.github.io/vld/integrations/database/diesel) · [api](https://docs.rs/vld-diesel) |
| [`vld-sea`](https://crates.io/crates/vld-sea) | SeaORM ActiveModel validation | [guide](https://s00d.github.io/vld/integrations/database/sea-orm) · [api](https://docs.rs/vld-sea) |
| [`vld-surrealdb`](https://crates.io/crates/vld-surrealdb) | SurrealDB document validation | [guide](https://s00d.github.io/vld/integrations/database/surrealdb) · [api](https://docs.rs/vld-surrealdb) |
| **Messaging** | | |
| [`vld-redis`](https://crates.io/crates/vld-redis) | Validated Redis values (sync and async) | [guide](https://s00d.github.io/vld/integrations/messaging/redis) · [api](https://docs.rs/vld-redis) |
| [`vld-lapin`](https://crates.io/crates/vld-lapin) | Validated RabbitMQ payloads | [guide](https://s00d.github.io/vld/integrations/messaging/lapin) · [api](https://docs.rs/vld-lapin) |
| **Config and CLI** | | |
| [`vld-config`](https://crates.io/crates/vld-config) | config-rs / figment validation | [guide](https://s00d.github.io/vld/integrations/config-cli/config) · [api](https://docs.rs/vld-config) |
| [`vld-clap`](https://crates.io/crates/vld-clap) | clap argument validation | [guide](https://s00d.github.io/vld/integrations/config-cli/clap) · [api](https://docs.rs/vld-clap) |
| **UI** | | |
| [`vld-dioxus`](https://crates.io/crates/vld-dioxus) | Shared validation for Dioxus server and WASM | [guide](https://s00d.github.io/vld/integrations/ui/dioxus) · [api](https://docs.rs/vld-dioxus) |
| [`vld-leptos`](https://crates.io/crates/vld-leptos) | Shared validation for Leptos server and WASM | [guide](https://s00d.github.io/vld/integrations/ui/leptos) · [api](https://docs.rs/vld-leptos) |
| [`vld-tauri`](https://crates.io/crates/vld-tauri) | Tauri command and event payloads | [guide](https://s00d.github.io/vld/integrations/ui/tauri) · [api](https://docs.rs/vld-tauri) |
| **Shared** | | |
| [`vld-http-common`](https://crates.io/crates/vld-http-common) | HTTP helpers shared by the web crates | [guide](https://s00d.github.io/vld/integrations/shared/http-common) · [api](https://docs.rs/vld-http-common) |
| [`vld-integration-common`](https://crates.io/crates/vld-integration-common) | DB / form helpers shared by the integration crates | [guide](https://s00d.github.io/vld/integrations/shared/integration-common) · [api](https://docs.rs/vld-integration-common) |

## Minimum supported Rust version

The core crate needs Rust **1.85** with every feature. Some integration features need a newer compiler; see [MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md).

## Contributing

See the [contributing guide](https://s00d.github.io/vld/guides/contributing) and the [changelog](https://github.com/s00d/vld/blob/main/CHANGELOG.md). Upgrading from 0.4: [migration guide](https://s00d.github.io/vld/guides/migration).

## License

MIT
