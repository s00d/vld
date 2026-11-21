[![Crates.io](https://img.shields.io/crates/v/vld?style=for-the-badge)](https://crates.io/crates/vld)
[![Downloads](https://img.shields.io/crates/d/vld?style=for-the-badge)](https://crates.io/crates/vld)
[![docs.rs](https://img.shields.io/badge/docs-docs.rs-blue?style=for-the-badge)](https://docs.rs/vld)
[![Guide](https://img.shields.io/badge/guide-s00d.github.io-blue?style=for-the-badge)](https://s00d.github.io/vld/)
[![CI](https://img.shields.io/github/actions/workflow/status/s00d/vld/ci.yml?branch=main&label=CI&style=for-the-badge)](https://github.com/s00d/vld/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![GitHub issues](https://img.shields.io/github/issues/s00d/vld?style=for-the-badge)](https://github.com/s00d/vld/issues)
[![GitHub stars](https://img.shields.io/github/stars/s00d/vld?style=for-the-badge)](https://github.com/s00d/vld/stargazers)
[![Donate](https://img.shields.io/badge/Donate-Donationalerts-ff4081?style=for-the-badge)](https://www.donationalerts.com/r/s00d88)

# vld

**Type-safe runtime validation for Rust, inspired by [Zod](https://zod.dev).**

Describe the shape of your data once, parse untrusted input into typed Rust values, and get every problem back with its path and a stable error code. The same schemas produce JSON Schema / OpenAPI and plug into web frameworks, databases, queues, config and CLI crates.

**Documentation: [s00d.github.io/vld](https://s00d.github.io/vld/)** · API reference: [docs.rs/vld](https://docs.rs/vld)

```toml
[dependencies]
vld = "0.8"
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
- **LLM structured outputs and tools** (feature `llm`): schemas for OpenAI / Anthropic / Gemini with provider limits checked, JSON extracted from model replies, validation errors as retry feedback; MCP (`rmcp`) and rig tool adapters in `vld-web`.
- **Small core**: only `serde` and `serde_json` by default; heavier validators are opt-in features.
- **Integrations** for 10 web frameworks, OpenAPI tools, databases, queues, config, CLI and UI in four crates, one module per library behind a feature, with one error format and safe defaults (2 MiB body limits, 415 / 422 handling).

## Crates

Released together with one version. No feature is on by default; any set of features, including several majors of one library, can be enabled together.

| Crate | Modules (features) | Docs |
|---|---|---|
| [`vld`](https://crates.io/crates/vld) | core schemas, macros, errors, i18n, JSON Schema; `form` (server functions, Leptos / Dioxus), `ts` (`ts`: Zod / Valibot / TypeScript), `fake` (`fake`: test data), `llm` (`llm`: LLM schemas, reply parsing, feedback) | [guide](https://s00d.github.io/vld/getting-started/introduction) · [api](https://docs.rs/vld) |
| [`vld-derive`](https://crates.io/crates/vld-derive) | `#[derive(Validate)]`, used through `vld`'s `derive` feature | [guide](https://s00d.github.io/vld/structs/derive) · [api](https://docs.rs/vld-derive) |
| [`vld-web`](https://crates.io/crates/vld-web) | `axum`, `actix`, `ntex3` / `ntex4` (`ntex-3` / `ntex-4`), `poem`, `rocket`, `salvo`, `warp`, `rama`, `tower`, `tonic`, `async_graphql`, `tungstenite`, `axum::ws` (`axum-ws`), `lambda`, `rmcp`, `rig`; `common` | [guide](https://s00d.github.io/vld/integrations/web/overview) · [api](https://docs.rs/vld-web) |
| [`vld-openapi`](https://crates.io/crates/vld-openapi) | `utoipa5` / `utoipa6` (`utoipa-5` / `utoipa-6`), `schemars0` / `schemars1` (`schemars-0` / `schemars-1`, also aide), `poem_openapi` (`poem-openapi`), `validator` | [guide](https://s00d.github.io/vld/integrations/openapi/utoipa) · [api](https://docs.rs/vld-openapi) |
| [`vld-data`](https://crates.io/crates/vld-data) | `sqlx08` / `sqlx09` (`sqlx-0_8` / `sqlx-0_9`), `diesel`, `sea1` / `sea2` (`sea-orm-1` / `sea-orm-2`), `surrealdb`, `redis0` / `redis1` (`redis-0` / `redis-1`), `lapin2` / `lapin3` / `lapin4` (`lapin-2` / `-3` / `-4`), `bson2` / `bson3` (`bson-2` / `bson-3`), `postgres_types` (`tokio-postgres`), `clickhouse`, `rumqttc`, `nats`, `centrifuge`, `csv` | [guide](https://s00d.github.io/vld/integrations/database/sqlx) · [api](https://docs.rs/vld-data) |
| [`vld-app`](https://crates.io/crates/vld-app) | `config` (`config-rs`, `figment`), `clap` (`clap`), `tauri` | [guide](https://s00d.github.io/vld/integrations/config-cli/config) · [api](https://docs.rs/vld-app) |

```toml
vld-web = { version = "0.8", features = ["axum"] }
vld-data = { version = "0.8", features = ["sqlx-0_8", "postgres"] }
```

```rust
use vld_web::axum::VldJson;
```

Moved in 0.8: the former per-integration crates (`vld-axum`, `vld-sqlx`, `vld-ts`, …) are modules of these crates; see the [migration guide](https://s00d.github.io/vld/guides/migration).

## Minimum supported Rust version

`vld` needs Rust **1.90** with every feature. Some integration features need a newer compiler; see [MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md).

## Contributing

See the [contributing guide](https://github.com/s00d/vld/blob/main/CONTRIBUTING.md) and the [changelog](https://github.com/s00d/vld/blob/main/CHANGELOG.md). Upgrading: [migration guide](https://s00d.github.io/vld/guides/migration).

## Support

If vld helps you, you can support it via [DonationAlerts](https://www.donationalerts.com/r/s00d88).
Bugs and ideas: [issues](https://github.com/s00d/vld/issues).

## License

MIT
