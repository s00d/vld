# Minimum supported Rust versions

Every crate's `rust-version` is the Rust its **default features** need.
Some optional features (mostly the newer major of a dual-major integration)
need more; they are listed below.

The numbers assume the MSRV-aware resolver picks your dependency versions:
Cargo 1.84+ does that by default for packages with `resolver = "3"` /
edition 2024, otherwise set it once:

```sh
CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback cargo update
# or in .cargo/config.toml: [resolver] incompatible-rust-versions = "fallback"
```

With newest-version resolution some dependencies need a newer compiler
(for example `image` 0.25.10 needs 1.88, `uuid` 1.29 needs 1.89).

`just msrv` (weekly in CI) checks every crate on its `rust-version` and
runs the core test suite with every feature on 1.85; each check resolves a
lockfile for that crate alone, as a downstream project would.

## Core

| crate | features | Rust |
|---|---|---|
| `vld` | default and every feature (`derive`, `regex`, `openapi`, `diff`, `chrono`, `jiff`, `time`, `decimal`, `net`, `file`, `file-advanced`, `string-advanced`) | 1.85 |
| `vld-derive`, `vld-http-common`, `vld-integration-common` | all | 1.85 |

## Integrations

| crate | features | Rust |
|---|---|---|
| `vld-actix`, `vld-axum`, `vld-poem`, `vld-rocket`, `vld-tower`, `vld-warp` | all | 1.85 |
| `vld-salvo` | all (Salvo 1.x) | **1.94** |
| `vld-rama` | all (Rama 0.4) | **1.96** |
| `vld-ntex` | `ntex-4` (default) | **1.97** |
| `vld-ntex` | `ntex-3` | 1.88 ¹ |
| `vld-tonic`, `vld-clap`, `vld-config`, `vld-ts`, `vld-fake`, `vld-tauri`, `vld-leptos`, `vld-dioxus`, `vld-surrealdb` | all | 1.85 |
| `vld-diesel` | `sqlite` (default), `postgres`, `mysql` | 1.85 |
| `vld-sqlx` | `sqlx-0_8` (default) + any backend | 1.85 |
| `vld-sqlx` | `sqlx-0_9` | **1.94** |
| `vld-sea` | `sea-orm-1` (default) + any `with-*` | 1.85 |
| `vld-sea` | `sea-orm-2` | **1.94** |
| `vld-utoipa` | `utoipa-5` (default) | 1.85 |
| `vld-utoipa` | `utoipa-6` | **1.88** |
| `vld-schemars`, `vld-aide` | `schemars-0` (default), `schemars-1` | 1.85 |
| `vld-redis` | `redis-0` (default), `redis-1`, `aio` | 1.85 |
| `vld-lapin` | `lapin-2` (default), `lapin-3` | 1.85 |
| `vld-lapin` | `lapin-4` | **1.88** |

¹ The crate declares 1.97 (its default, ntex 4), so Cargo refuses older
compilers; with `default-features = false, features = ["ntex-3"]` it builds
on 1.88 when you pass `--ignore-rust-version`.

The workspace floor is 1.85 (edition 2021 crates, tested in CI); several
crates' own dependency trees would go lower, but that is not tested.
