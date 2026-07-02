# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [0.4.0] - 2026-07-02

### Added

- Unified `vld-utoipa` OpenAPI API: `#[into_params(parameter_in = ...)]` on structs + single `impl_to_schema!` for `ToSchema` and `IntoParams` (fixes query/path validation constraints in OpenAPI — [#3](https://github.com/s00d/vld/issues/3))
- Optional `jiff` date/datetime backend (`jiff::civil::Date`, `jiff::Timestamp`) with full `ZDate` / `ZDateTime` API parity
- Optional `time` date/datetime backend (`time::Date`, `time::OffsetDateTime` UTC) with full API parity
- Integration tests and CI matrix for `jiff` and `time` backends
- `crates/vld-axum/examples/axum_openapi.rs` end-to-end Axum + utoipa example
- `tests/jiff_dates.rs` and `tests/time_dates.rs`

### Changed

- Split date primitives into `src/primitives/date/{chrono,jiff,time}.rs` with backend priority `chrono` > `jiff` > `time` when multiple features are enabled (`--all-features` safe)
- `vld-utoipa`: deprecated legacy macros (`impl_to_schema_query!`, `impl_into_params!`, suffix forms) with migration docs
- Framework READMEs: OpenAPI sections for query/path params with unified API
- Workspace `vld-utoipa` dependency uses `{ workspace = true }`

### Deprecated

- `impl_into_params!`, `impl_to_schema_query!`, `impl_to_schema_path!`, and `impl_to_schema!(T, query|path|...)` — use `#[into_params]` + `impl_to_schema!(T)` instead

## [0.3.0] - 2026-03-20



### Added


- Add native vld transport integrations

- Add bytes schema and stricter datetime validation

- Add timezone-aware datetime and file schema validation

- Extend file storage access and refresh formatting

- Add advanced typed schemas and cross-crate format support


### Changed


- Simplify Zod/Valibot generation API

- Remove unused full-file generation internals

- Gate heavy validators behind opt-in features


### Fixed


- Run per-crate preflight safely

- Satisfy strict clippy assertions

- Replace approximate float constant in test

- Keep float coercion expectation exact

- Remove unused prelude imports

- Remove unused prelude imports

- Remove unused VldSchema import in tests

- Align prelude trait imports for optional combinators

- Stabilize clippy around optional combinator trait imports


### chore


- Automate changelog generation with git-cliff


## [0.2.0] - 2026-03-19



### Added


- Add tonic gRPC integration for vld validation

- Add Leptos integration for shared server/WASM validation

- Add SQLx integration for vld validation

- Add Dioxus integration for shared server/WASM validation

- Add ntex web framework integration

- Add aide/schemars integration for OpenAPI generation

- Add SurrealDB integration for JSON document validation

- Add bidirectional bridge between vld and schemars

- Add reverse direction — schemars → vld validation

- Add nested schema auto-registration for OpenAPI

- Auto-register nested schemas in utoipa components

- Auto-register nested schemas in schemars definitions


### Changed


- Replace standalone functions with macro+trait API

- Add nested schema auto-registration section to README

- Add dead code allowance for unused schema methods


### Fixed


- Formatting, clippy warnings, and missing imports across workspace


### chore


- Register vld-tonic and vld-leptos in workspace, update root README

- Register vld-sqlx in workspace, update root README

- Register vld-dioxus in workspace, update root README

- Register vld-ntex in workspace, update root README

- Register vld-aide in workspace, update root README

- Register vld-surrealdb in workspace, update root README

- Register vld-schemars in workspace, update root README


## [0.1.3] - 2026-03-19



### Added


- Add health check endpoint and response schemas

- Generate json_schema() for derive(Validate), enabling utoipa integration


### chore


- Bump workspace version to 0.1.2

- Remove .idea/ from git tracking

