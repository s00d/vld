# Contributing

## Layout

| Path | Contents |
|---|---|
| `crates/vld` | core crate: `src/`, `tests/it/` (one test binary), `benches/` (Criterion) |
| `crates/vld-*` | integration crates |
| `fuzz/` | `cargo-fuzz` targets (own workspace) |
| `examples/playground` | an app that uses many crates together |
| `docs/` | documentation site (Nuxt Content) |

All crates share one version and are released together (`PUBLISHING.md`). How tests are written: `TESTING.md`.

## CI recipes

CI runs `just` recipes; the same commands work locally.

```bash
cargo install just cargo-hack cargo-deny cargo-shear cargo-semver-checks cargo-sweep
just          # list recipes
just ci       # what CI runs on every push
```

| Recipe | What it does |
|---|---|
| `fmt` | `cargo fmt --check` |
| `lint` | `cargo shear`, then clippy `-D warnings`: workspace, each `vld` feature alone and all together, feature pairs of integration crates |
| `test` | tests and doctests with the same feature sets as `lint` |
| `docs` | `cargo doc` with `-D warnings` |
| `deny` | `cargo deny` (advisories, licenses, duplicates) |
| `package` | `cargo package` dry run |
| `fuzz` | the fuzz targets compile |
| `playground` | runs the playground |
| `trim` | caps `target/` at 10 GB with `cargo sweep` (oldest artifacts first) |
| `wasm` | `vld`, `vld-leptos`, `vld-dioxus` for `wasm32` |
| `powerset` | every pair of `vld` features compiles (slow) |
| `msrv` | every crate on its `rust-version` (`MSRV.md`) |
| `minimal` | lowest allowed dependency versions (nightly) |
| `semver` | `cargo semver-checks` against the latest crates.io release |
| `clean` | `cargo clean` and `fuzz/target` |

`wasm`, `powerset`, `msrv`, `minimal` and the Windows / macOS tests run only before a release (Actions → CI → Run workflow).

Crates with one feature per supported major of a dependency (`redis-0` / `redis-1`, `sqlx-0_8` / `sqlx-0_9`, …) are listed in `feature_crates` in the `justfile`; the majors never combine, so never use `--all-features` on those crates. Builds are `--locked` and use the `ci` profile (`target/ci`). diesel's `postgres` / `mysql` need `libpq` / `libmysqlclient`.

## Snapshots

`vld` (its `ts` module too: `tests/it/ts/snapshots/`), `vld-utoipa` and `vld-aide` use [insta](https://insta.rs) snapshots (`tests/it/snapshots/`). After an intended output change:

```bash
cargo insta test -p vld --all-features
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
