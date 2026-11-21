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
(newer releases of a dependency may raise its `rust-version`).

`just msrv` (before each release) builds each crate on its `rust-version`, and the
features below on theirs, in a project that depends on that crate alone.

Every crate declares `rust-version = "1.90"`, the Rust its default (empty)
feature set needs, and every feature builds on it unless listed below; Cargo
names the dependency and its `rust-version` when your compiler is older.

| crate | features | Rust |
|---|---|---|
| `vld`, `vld-derive`, `vld-openapi`, `vld-app` | every feature | 1.90 |
| `vld-web` | every feature except the ones below | 1.90 |
| `vld-web` | `salvo` | **1.94** |
| `vld-web` | `rama` | **1.96** |
| `vld-web` | `ntex-4` | **1.97** |
| `vld-web` | `rig` | **1.97.1** |
| `vld-data` | every feature except the ones below | 1.90 |
| `vld-data` | `sqlx-0_9`, `sea-orm-2` | **1.94** |

CI builds and lints with Rust 1.100 (`RUST_TOOLCHAIN` in `.github/workflows/ci.yml`);
`just msrv` (the manual `extended` job) checks the floors above.
