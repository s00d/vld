# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [Unreleased]

## [0.4.5] - 2026-10-01
### Fixed

- HTTP adapters: enforce body size limits (axum `Bytes`/`DefaultBodyLimit`, warp `content_length_limit`, poem `into_bytes_limit`, rama `Body::limited`) and return **413** on overflow
- `vld-warp` `handle_rejection`: map `LengthRequired` → **411** and `PayloadTooLarge` → **413** (no longer collapse to 404)
- actix/ntex/salvo: remapped payload overflow to **413** instead of always 422
- poem: malformed JSON uses `format_vld_error` (422); rama `ValidateJsonLayer` aligned to 422 + validation body
- `vld-http-common`: consistent issue path formatting (`.user.age`) for `format_issues` / `format_issues_with_code`

## [0.4.4] - 2026-10-01
### Changed

- Bump optional core pins: `infer` 0.22, `rust_decimal` 1.43, `ipnet` 2.12.2, `phonenumber` 0.3.10
- `vld-salvo`: Salvo **1.x** only (`salvo = "1"`); crate MSRV 1.94 (cross-major `>=0.89,<2` dual-loads `salvo_core` / breaks `Extractible`)
- `vld-sea`: dual features `sea-orm-1` (default) / `sea-orm-2`; `Value::Enum` → JSON string on 2.x; entity-macro integration tests stay on 1.x; `sea-orm-2` requires Rust **1.94+**
- `vld-utoipa`: dual features `utoipa-5` (default) / `utoipa-6` (CI tests both); `utoipa-6` requires Rust **1.88+**

### Deferred

- `syn` 3, ntex 4 (beta), aide 0.16 (alpha) — wait for stable / lower churn

## [0.4.3] - 2026-10-01

### Fixed

- Omit `.optional()` / `.nullish()` / `.with_default()` fields from JSON Schema `required` (and keep `.nullable()` required) across `schema!`, `#[derive(Validate)]`, and `ZObject` (#6)
- Forward `JsonSchema::is_required` through describe/refine/message/transform/catch wrappers
- `vld-utoipa::json_schema_to_params` uses the object `required` array (so `.nullable()` query/path params stay required; oneOf+null no longer forces `Required::False`)

## [0.4.2] - 2026-09-27



### Added


- Add vld-rama for Rama HTTP validation


### Fixed


- Bring VldSchema into schema! OpenAPI helpers

- Drop unused VldSchema imports after schema! fix


## [0.4.1] - 2026-09-26



### Fixed


- Register transitive nested OpenAPI schemas 

- Alias NestedSchemaCollectFn for clippy type_complexity


## [0.4.0] - 2026-07-02



### Changed


- Refresh crate links and dependency versions

- Enhance build and test process for vld-diesel


### Fixed


- Make path-within check deterministic in CI

- Restore integration examples and tests build


### chore


- Downgrade workspace and crate versions to 0.3.0

- Update dependencies and versions in Cargo files

- Normalize integration dependency compatibility

- Implement workspace-wide pre-release hook and update preflight script

- Update pre-release hook and streamline release preflight script


### ci


- Unify workspace checks and restore crate version badges


### release


- V0.4.0 — unified utoipa OpenAPI, jiff/time date backends


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

