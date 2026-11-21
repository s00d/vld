# Testing

Behaviour is tested the way users use vld: build a schema (or derive / `schema!` a type), parse
JSON, read the result, the issues, the JSON Schema, the generated code, the framework response.
Tests go through the public API only.

## Kinds

| Kind | What | Example |
|---|---|---|
| **model** (proptest) | Random inputs (and random schema trees) checked against a small model of the documented rules: the model predicts the output or the exact `(path, code)` issue list. On failure proptest shrinks to the smallest case. | `crates/vld/tests/it/model_schema.rs` |
| **story** | One user journey per test, start to end, with a `Given / When / Then` doc comment: e.g. derive an API request type, parse a body, read the issues, translate them, emit OpenAPI. A story checks every step's observable result, not one function. | `crates/vld/tests/it/story_*.rs` |
| **corpus** | A table of inputs with expected verdicts (string formats, dates, …) or a differential comparison against another implementation (JSON Schema validator, serde, Zod in node). | `crates/vld/tests/it/corpus_*.rs` |

## Layout

Each crate has **one** integration test binary, `tests/it/main.rs`, whose modules are the model,
story and corpus files, plus `tests/it/common/` for helpers. The crate and its dependencies link
once. No `#[cfg(test)] mod tests` unit tests in `src/` (doctests stay: they are documentation),
except checks the public API cannot make: `crates/vld/src/error/code.rs` and `error/i18n.rs`
(the exhaustive match over the `#[non_exhaustive]` `IssueCode`, the template-table walks) and
`crates/vld-derive/src/` (rename helpers and the derive's compile errors; no `trybuild`).

Group crates (`vld-web`, `vld-openapi`, `vld-data`, `vld-app`): one directory per integration,
`crates/<group>/tests/it/<integration>/`, compiled by `main.rs` only with that integration's feature
(`#[cfg(feature = "axum")] mod axum;`). A dependency with several majors has one wrapper file per
major (`tests/it/sqlx08.rs`, `sqlx09.rs`, `ntex3.rs`, …) that compiles the shared directory against
that major, the same way as `src/`. Tests are stories through the real framework (a request through
the router, a row through the database layer, a config file through the loader), covering every
extractor / entry point the module exports, the error response and the OpenAPI output. Every
`vld-web` module also runs the shared HTTP conformance suite (`crates/vld-web/tests/it/conformance/`).

## Rules

- Compare outputs, issue codes and paths, status codes, JSON Schema and generated code. Compare
  message texts only in stories about messages (custom messages, i18n).
- A new regression is a new step in a story or a case the model now generates, never a new
  one-function test.
- Mutation testing runs once, after the tests are in shape. The survivor list is a direction for
  new tests, not a list to drive to zero, and is never re-run to confirm. A survivor worth a test
  is covered by extending a model or a story, never by a one-function test; a mutant no input can
  observe is listed in `.cargo/mutants.toml` `exclude_re` with the reason.
- No timing: no sleeps, no real-time limits. Per-test timeouts are in `.config/nextest.toml`.
- Logs of long runs go to `target/logs/<task>.log`.

## Running

```bash
cargo nextest run -p vld --all-features                # one crate
cargo nextest run -p vld --all-features -E 'test(/^story_api_request::/)'   # one area
cargo nextest run -p vld-web --features axum           # one integration
just test-group web                                    # one group, as its CI job runs it
just test                                              # every group (CONTRIBUTING.md)
```

Snapshots are insta files in `crates/vld/tests/it/snapshots/`, `crates/vld/tests/it/ts/snapshots/`
and `crates/vld-openapi/tests/it/{utoipa,schemars}/snapshots/`: `cargo insta test -p <crate>
--all-features`, then `cargo insta review`. The `vld::ts` Node cross-check runs only
with `VLD_TS_NODE_DIR` set (see `CONTRIBUTING.md`).

## Mutation testing

[cargo-mutants](https://mutants.rs) edits the code one small change at a time and runs the tests
against each mutant; a mutant the tests still pass on ("missed") marks behaviour no test pins down.
Local only, one scoped run at a time (about 5 s per mutant):

```bash
mkdir -p target/mutants/tmp
CARGO_BUILD_JOBS=6 TMPDIR=$PWD/target/mutants/tmp cargo mutants -p vld --all-features \
  --file crates/vld/src/primitives/number/int.rs --output target/mutants/int -- --test it
```

Results: `<output>/mutants.out/{missed,caught,timeout,unviable}.txt`. Remove `target/mutants`
afterwards.
