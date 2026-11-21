# Architecture

How the workspace is split into crates, how one crate supports several majors
of a dependency, and how it is tested and released. Read this before adding
an integration or a dependency major.

## Crates

Six published crates, released together with one version:

| crate | what it holds | depends on |
|---|---|---|
| `vld` | schemas, errors, JSON Schema, `schema!` / `impl_rules!`, form-style field errors; optional TypeScript / Zod / Valibot codegen (`ts`) and fake data (`fake`) | serde, serde_json, optional backends |
| `vld-derive` | `#[derive(Validate)]` (a proc-macro must be its own crate) | syn, quote |
| `vld-web` | HTTP extractors and error responses for every web framework, plus the shared HTTP logic | `vld`, the frameworks (optional) |
| `vld-openapi` | utoipa and schemars (aide) bridges, the JSON Schema validator | `vld/openapi`, utoipa / schemars (optional) |
| `vld-data` | databases and message queues: sqlx, diesel, sea-orm, surrealdb, redis, lapin | `vld`, the drivers (optional) |
| `vld-app` | application edges: config files (config-rs, figment), clap, tauri | `vld`, the libraries (optional) |

`vld` depends on no framework, so a user's build compiles it in parallel with
the framework, and an integration crate is small glue on top of both.

Not published: `examples/playground`, `fuzz` (own workspace).

### Module per integration, feature per dependency

Every integration is a public module of its group crate, compiled only when
its feature is on. No feature is on by default: a user names what they use.

```toml
vld-web = { version = "0.8", features = ["axum"] }
```

```rust
use vld_web::axum::{VldJson, VldQuery};
```

A module exposes the same items the old single-integration crate did
(`VldJson`, `VldQuery`, …, its error type, `prelude`). Code shared by several
modules of one group lives in a private or `#[doc(hidden)]` module of that
group (`vld_web::common`, `vld_data::db`).

### Several majors of one dependency

A dependency supported in more than one major (ntex 3 / 4, utoipa 5 / 6,
schemars 0.9 / 1, sqlx 0.8 / 0.9, sea-orm 1 / 2, redis 0.x / 1, lapin 2 / 3 / 4)
is an optional dependency per major, renamed (`ntex3`, `ntex4`), with one
feature and one module per major. The majors are additive: any set of them can
be enabled together (docs.rs builds with `all-features`, and a dependency graph
that enables two majors still compiles).

The implementation is written once, against an alias, and compiled once per
major:

```text
src/ntex3.rs         use ::ntex3 as ntex;  #[path = "ntex/shared.rs"] mod shared;
                     pub use shared::*;    mod compat { /* ntex 3 only */ }
src/ntex4.rs         same with ::ntex4
src/ntex/shared.rs   the implementation: `use super::ntex;`, `super::compat::…`
```

`shared.rs` never tests a major feature with `cfg`: both majors may be on at
once. What differs between majors (a trait signature, a type alias, a match
arm only one major has) lives in the wrapper's `compat` module, behind a
function, type or macro both `compat`s define with the same name. Features
that apply to every major (`ntex-compress`, sea-orm `with-*`, `postgres`)
forward to each optional dependency with `dep?/feature` and may be tested with
`cfg` in `shared.rs`.

A dependency with one public API across a version range (warp 0.3–0.4, tonic
0.12–0.14, config 0.14–0.15) keeps one module and a range requirement.

### Macros

`#[macro_export]` puts a macro at the crate root, so two modules cannot both
export `impl_to_schema!`. Each macro is exported under a unique hidden name and
re-exported from its module:

```rust
#[doc(hidden)] #[macro_export]
macro_rules! __utoipa5_impl_to_schema { /* uses $crate::utoipa5::… */ }
pub use crate::__utoipa5_impl_to_schema as impl_to_schema;   // in mod utoipa5
```

Users write `vld_openapi::utoipa5::impl_to_schema!(T)`. Macros used only inside
a crate are not exported.

## Where the former crates went

| 0.7 crate | 0.8 location | feature |
|---|---|---|
| `vld-http-common` | `vld_web::common` (public, for custom adapters) | — |
| `vld-axum`, `vld-actix`, `vld-poem`, `vld-rocket`, `vld-salvo`, `vld-warp`, `vld-rama`, `vld-tower`, `vld-tonic` | `vld_web::<name>` | `<name>` |
| `vld-ntex` | `vld_web::ntex3`, `vld_web::ntex4` | `ntex-3`, `ntex-4`, `ntex-compress` |
| `vld-utoipa` | `vld_openapi::utoipa5`, `::utoipa6` | `utoipa-5`, `utoipa-6` |
| `vld-schemars`, `vld-aide` | `vld_openapi::schemars0`, `::schemars1`; the validator is `vld_openapi::validator` (always on); aide uses the schemars module of its schemars major | `schemars-0`, `schemars-1` |
| `vld-ts` | `vld::ts` | `vld/ts` |
| `vld-fake` | `vld::fake` | `vld/fake` |
| `vld-sqlx` | `vld_data::sqlx08`, `::sqlx09` | `sqlx-0_8`, `sqlx-0_9` + `postgres` / `mysql` / `sqlite` |
| `vld-diesel` | `vld_data::diesel` | `diesel` + `postgres` / `mysql` / `sqlite` |
| `vld-sea` | `vld_data::sea1`, `::sea2` | `sea-orm-1`, `sea-orm-2` + `with-*`, `postgres-array` |
| `vld-surrealdb` | `vld_data::surrealdb` | — (no dependency) |
| `vld-redis` | `vld_data::redis0`, `::redis1` | `redis-0`, `redis-1`, `redis-aio`, `redis-aio-tokio` |
| `vld-lapin` | `vld_data::lapin2`, `::lapin3`, `::lapin4` | `lapin-2`, `lapin-3`, `lapin-4` |
| `vld-config` | `vld_app::config` | `config-rs`, `figment` |
| `vld-clap` | `vld_app::clap` | `clap` |
| `vld-tauri` | `vld_app::tauri` | — (no dependency) |
| `vld-leptos`, `vld-dioxus` | `vld::form` (server-function validation, no framework code) | — |
| `vld-integration-common` | `vld::form`, `vld::error::{format_path, flatten_issues}`; the DB wrapper macros in `vld_data::db` | — |

The backend features `postgres` / `mysql` / `sqlite` apply to every enabled
driver (`sqlx08?/postgres`, `diesel?/postgres`, …). diesel's postgres and
mysql backends link libpq / libmysqlclient.

## Rust versions

Every crate declares `rust-version = "1.85"`: what its default (empty) feature
set needs. A feature whose dependency needs a newer compiler (ntex 4: 1.97,
rama: 1.96, salvo / sqlx 0.9 / sea-orm 2: 1.94, utoipa 6 / lapin 4 / ntex 3:
1.88) is listed in MSRV.md; cargo reports the dependency's `rust-version` when
a user's compiler is older.

## Tests

One integration test binary per crate, `tests/it/` (method: TESTING.md). In a
group crate every integration is a module directory compiled with its
feature:

```text
crates/vld-web/tests/it/main.rs        #[cfg(feature = "axum")] mod axum; …
crates/vld-web/tests/it/conformance/   the shared HTTP suite (was vld-http-common's
                                       `conformance` feature, now test-only)
crates/vld-web/tests/it/ntex/          shared ntex tests, compiled by ntex3.rs / ntex4.rs
                                       wrappers the same way as src
```

`cargo test -p <group> --all-features` builds the group once and runs every
integration and every major. `cargo hack check -p <group> --each-feature
--no-dev-deps` proves each feature compiles on its own.

## CI and release

- CI runs one job per crate group (`core` = vld + vld-derive, `web`,
  `openapi`, `data`, `app`) in parallel, each with its own cache, plus a small
  job for fmt / docs / deny / package / fuzz / playground. Each group job runs
  `just group <name>`: clippy and tests with all features, the `--each-feature`
  check, and (core only) clippy per feature and tests without default
  features. Feature pairs, MSRV toolchains, other OSes and minimal versions
  stay in the manual `extended` job.
- Releases: `cargo release minor --workspace` publishes the six crates in
  dependency order (PUBLISHING.md). The release hook only rewrites the
  changelog; it requires a green CI run on the release commit instead of
  re-running the checks locally.
