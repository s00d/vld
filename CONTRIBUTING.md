# Contributing

## Layout

| Path | Contents |
|---|---|
| `crates/vld` | core crate: `src/`, `tests/it/` (one test binary), `benches/` (Criterion) |
| `crates/vld-derive` | `#[derive(Validate)]` |
| `crates/vld-web`, `vld-openapi`, `vld-data`, `vld-app` | integration groups: one module per integration in `src/`, tests in `tests/it/<integration>/` |
| `fuzz/` | `cargo-fuzz` targets (own workspace) |
| `examples/playground` | an app that uses many crates together |
| `docs/` | documentation site (Nuxt Content) |

All crates share one version and are released together (`PUBLISHING.md`). How the crates are split and how several majors of a dependency are supported: `ARCHITECTURE.md`. How tests are written: `TESTING.md`.

## CI recipes

CI runs `just` recipes; the same commands work locally.

```bash
cargo install just cargo-hack cargo-deny cargo-shear cargo-semver-checks cargo-sweep
just          # list recipes
just ci       # what CI runs on every push
```

Groups: `core` (`vld` + `vld-derive`), `web`, `openapi`, `data`, `app` (`vld-<group>`). CI runs one job per group (`just group <name>`) plus one for `misc`.

| Recipe | What it does |
|---|---|
| `group <name>` | `lint-group` then `test-group`: what one CI job runs |
| `lint-group <name>` | clippy `-D warnings` with all features, then each feature alone (`cargo hack --each-feature`) |
| `test-group <name>` | tests and doctests with all features (`core`: also without default features) |
| `lint`, `test` | `lint-group` / `test-group` for every group |
| `misc` | `fmt`, `docs`, `deny`, `package`, `fuzz`, `playground`, `cargo shear` |
| `fmt` | `cargo fmt --check` |
| `docs` | `cargo doc` with `-D warnings`, all features |
| `deny` | `cargo deny` (advisories, licenses, duplicates) |
| `package` | `cargo package` dry run |
| `fuzz` | the fuzz targets compile |
| `playground` | runs the playground |
| `trim` | caps `target/` at 10 GB with `cargo sweep` (oldest artifacts first) |
| `wasm` | `vld` (with `vld::form`) for `wasm32` |
| `powerset` | every pair of `vld` features compiles (slow) |
| `msrv` | every crate on its `rust-version`, and the features that need more on theirs (`MSRV.md`) |
| `minimal` | lowest allowed dependency versions (nightly) |
| `semver` | `cargo semver-checks` against the latest crates.io release |
| `clean` | `cargo clean` and `fuzz/target` |

`wasm`, `powerset`, `msrv`, `minimal` and the Windows / macOS tests run only before a release (Actions → CI → Run workflow).

Every supported major of a dependency is its own feature (`redis-0` / `redis-1`, `sqlx-0_8` / `sqlx-0_9`, …) and the majors combine, so `--all-features` builds every integration and every major of a group at once. Builds are `--locked` and use the `ci` profile (`target/ci`). vld-data's diesel `postgres` / `mysql` need `libpq` / `libmysqlclient`.

## Snapshots

[insta](https://insta.rs) snapshots live in `crates/vld/tests/it/snapshots/`, `crates/vld/tests/it/ts/snapshots/` and `crates/vld-openapi/tests/it/{utoipa,schemars}/snapshots/`. After an intended output change:

```bash
cargo insta test -p vld --all-features            # or -p vld-openapi
cargo insta review
```

## vld::ts against Node

The generated Zod 3, Zod 4 and Valibot code is run under Node when `VLD_TS_NODE_DIR` points at a directory with `zod` (>= 3.25, for `zod/v3` and `zod/v4`) and, optionally, `valibot@1`; otherwise the check is skipped.

```bash
mkdir -p .ts-check && echo '{"private":true}' > .ts-check/package.json
npm --prefix .ts-check install zod@^3.25 valibot@1
VLD_TS_NODE_DIR=$PWD/.ts-check cargo test -p vld --all-features --test it ts::
```

## Fuzzing

```bash
cargo install cargo-fuzz
cd fuzz
cargo +nightly fuzz run iso_formats -- -max_total_time=60
```

Targets: `iso_formats`, `string_formats`, `url_decode`, `error_truncation`. Crashes land in `fuzz/artifacts/<target>/`.

## Documentation

```bash
cd docs && pnpm install
pnpm dev                                # http://localhost:3000
NUXT_APP_BASE_URL=/vld/ pnpm generate   # static build in .output/public
```

`CHANGELOG.md` and `MSRV.md` are copied into the site by `docs/scripts/sync.mjs`; edit the originals. Deployed to GitHub Pages by `.github/workflows/docs.yml`.

## Changelog

Add entries under `## [Unreleased]` at the top of `CHANGELOG.md`, and a section in the docs migration guide (`docs/content/7.guides/1.migration.md`) for anything breaking.
