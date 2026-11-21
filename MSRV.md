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

`just msrv` (before each release) builds each crate on its `rust-version`, and the
features below on theirs, in a project that depends on that crate alone.

Every crate declares `rust-version = "1.85"`, the Rust its default (empty)
feature set needs. A feature whose dependency needs more is listed here; Cargo
names the dependency and its `rust-version` when your compiler is older.

| crate | features | Rust |
|---|---|---|
| `vld` | every feature | 1.85 |
| `vld-derive` | — | 1.85 |
| `vld-web` | `axum`, `axum-ws`, `actix`, `poem`, `rocket`, `warp`, `tower`, `tonic`, `tungstenite`, `lambda` | 1.85 |
| `vld-web` | `async-graphql` | **1.89** |
| `vld-web` | `ntex-3`, `rmcp` | **1.88** |
| `vld-web` | `salvo` | **1.94** |
| `vld-web` | `rama` | **1.96** |
| `vld-web` | `ntex-4` | **1.97** |
| `vld-web` | `rig` | **1.97.1** |
| `vld-openapi` | `utoipa-5`, `schemars-0`, `schemars-1`, `poem-openapi` | 1.85 |
| `vld-openapi` | `utoipa-6` | **1.88** |
| `vld-data` | `sqlx-0_8`, `diesel`, `sea-orm-1` (+ `with-*`), `redis-0`, `redis-1`, `redis-aio*`, `lapin-2`, `lapin-3`, `bson-2`, `bson-3`, `tokio-postgres`, `rumqttc`, `centrifuge`, `csv`, `postgres` / `mysql` / `sqlite` | 1.85 |
| `vld-data` | `lapin-4` | **1.88** |
| `vld-data` | `nats`, `clickhouse` | **1.89** |
| `vld-data` | `sqlx-0_9`, `sea-orm-2` | **1.94** |
| `vld-app` | every feature | 1.85 |

The workspace floor is 1.85 (edition 2021 crates, tested in CI); several
crates' own dependency trees would go lower, but that is not tested.
