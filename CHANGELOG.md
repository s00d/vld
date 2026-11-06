# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [0.5.0] - 2026-11-06

### Breaking changes

- **Features**: the `std` feature is gone (everything it gated is always available); `serialize` / `deserialize` are deprecated no-ops (removed in 0.6); `diff` enables `openapi`.
- **Date backends are additive**: `ZDate` / `ZDateTime` are generic over the backend and each feature has its own module (`vld::chrono`, `vld::jiff`, `vld::time`). `vld::date()` / `vld::datetime()` and the prelude aliases exist only when exactly one backend is enabled. Previously the backend was picked silently (chrono > jiff > time).
- **Issue codes**: `IssueCode` has a typed variant for every built-in check (`InvalidLength`, `InvalidFormat`, `InvalidDate`, `NotMultipleOf`, `InvalidEnumValue`, `InvalidUnion`, `NotUnique`, `MissingField`, `FileTypeNotAllowed`, `SerializeError`, …); `Custom` is for user codes only. `TooSmall` / `TooBig` carry `origin: SizeOrigin` and a `serde_json::Value` bound instead of an `f64`. `IssueCode`, `StringValidation` and `DateKind` are `#[non_exhaustive]`.
- **Messages**: default messages come from one English template table; an absent required key reports `missing_field` (a present `null` stays `invalid_type`); `Display` no longer appends ", received X" to `invalid_type`.
- **Module layout**: core split into `schema/`, `error/`, `object/`, `macros/`, `openapi/` (old paths `vld::json_schema`, `vld::diff`, `vld::format`, `vld::i18n`, `vld::input` remain as aliases); the inherent `to_json_schema()` duplicates are gone (use the `JsonSchema` trait).
- **JSON Schema**: optional / nullable / nullish fields and unions emit `anyOf` (was `oneOf`); `ZNullable::is_required()` is `false`.
- **Caching**: `schema!`, `impl_validate_fields!`, `impl_rules!` and `#[derive(Validate)]` build field schemas once per process, so field schemas must be `Send + Sync + 'static`; `VldInput::to_json_value` returns `Cow<Value>`.
- **`#[derive(Validate)]`**: `validate_fields()` / `parse_lenient()` are generated only with `#[vld(lenient)]`; `#[serde(flatten)]` / `alias` are compile errors.
- **Web adapters**: every `VldJson` extractor answers **415** unless the content type is `application/json` or `application/*+json`; malformed JSON is **422** with the validation body (was 400); 422 issues carry a `code` field; query/path/form/header/cookie values fall back to the original text when the schema wants a string (`parse_coerced`).
- **`vld-tower` / `vld-rama`**: `validated()` returns `Option<T>`; the tower inner service receives `Request<ValidatedBody<B>>` (non-JSON bodies are no longer buffered).
- **`vld-warp`**: `handle_rejection` maps warp rejections to their real status (unknown → 500 instead of 404); bodies are read as a stream with a limit (413).
- **`vld-ntex`**: ntex **4** is the default (`ntex-4`); `default-features = false, features = ["ntex-3"]` keeps ntex 3. Crate MSRV 1.97.
- **`vld-ts`**: `with_default` emits `x.nullish().transform((v) => v ?? d)` (Zod) / `v.nullish(x, d)` (Valibot), matching vld, which accepts `null` there.
- **`vld-clap`**: `validate_or_exit` requires `clap::CommandFactory` and reports clap's real argument names; depends on clap 4.
- **Issue paths** in `vld-tauri`, `vld-leptos`, `vld-dioxus`, `vld-surrealdb` have no leading dot (`user.email`, `items[0]`).

### Migration from 0.4

- **Cargo features**: drop `std`, `serialize`, `deserialize` from your feature lists.
- **Dates**: with one backend nothing changes. With several, call `vld::chrono::date()`, `vld::jiff::datetime()`, `vld::time::time()` and name `vld::chrono::ZDate` etc.
- **Matching on issue codes**: replace `IssueCode::Custom { code: "…" }` checks for built-in errors with the typed variants; `TooSmall { minimum, .. }` now has `origin` and a JSON `minimum` (`minimum.as_f64()`). Add a `_ =>` arm (non-exhaustive enums). Prefer `IssueCode::key()` for stable string codes.
- **Messages / i18n**: tests that compared full English messages may need updating (`missing_field`, no ", received X"); custom translation tables can add the new keys (`missing_field`, `type.<name>`, `too_small.<origin>`, …), the specific key is tried before the generic one.
- **JSON Schema consumers**: expect `anyOf` instead of `oneOf` for optional / nullable fields and unions.
- **Web clients**: send `Content-Type: application/json`; handle 415 and 422 (malformed JSON), read the new `code` field.
- **Derive**: add `#[vld(lenient)]` where you call `validate_fields()` / `parse_lenient()`.
- **ntex 3 users**: `vld-ntex = { version = "0.5", default-features = false, features = ["ntex-3"] }`.
- **vld-ts**: regenerate client schemas; defaults now accept `null`.
- **vld-clap**: derive `clap::Parser` (or implement `CommandFactory`) for `validate_or_exit`, or call `validate_cli`.
- **tower / rama**: `validated()` returns `Option`; `try_validated` is deprecated.
- **MSRV**: see [MSRV.md](MSRV.md).

### Added

- Schemas: sized integers `vld::i8()`…`u64()`, `isize()`, `usize()`, `vld::f32()`; `vld::null()`, `vld::never()`, `vld::char()`; `vld::from_serde()`, `vld::from_str()`, `vld::enum_of()`; time-of-day `vld::{chrono,jiff,time}::time()` / `vld::time_of_day()`; `vld::record_with_keys()` and `ZRecord::ordered()`; tuples up to arity 12; string formats `base64url()` and `json()`; `ZInt::coerce()`; `ZDecimal` `gt` / `lt` / `multiple_of`.
- `parse_coerced`: schema-aware fallback for values coerced from text.
- `duration()` accepts fractional seconds (`1.5`, `"1.5s"`, `"PT1.5S"`).
- `ZFile::within(base)`; accessors on `ZLiteral`, `ZMap`, `ZPipe`, `ZDiscriminatedUnion`, `ZLazy`; `type_error()` on `ZBoolean` / `ZEnum` / `ZJsonValue`.
- Complete Russian, German and Spanish message tables; translated type names.
- `vld-integration-common` (new crate): shared DB column wrappers, errors and form helpers for the integration crates.
- `vld-http-common`: `VldHttpError`, shared JSON / form / header / cookie helpers, `require_json_content_type`, a `conformance` test suite (test-only feature).
- `vld-redis`: async `AsyncRedisConn` (`aio` feature); the selected redis crate is re-exported.
- `vld-ntex`: ntex 4 support.
- `vld-rocket`: `vld_413_catcher`, `vld_415_catcher`, `vld_catchers()`.
- `vld-diesel`: `VldFloat`, `VldBool`; wrappers usable in `Insertable` / `.filter()`.
- `vld-sea`: `with-json`, `with-chrono`, `with-uuid`, `with-rust_decimal`, `with-bigdecimal` features.
- `vld-clap`: `validate_cli`.
- `vld-ts`: named exports (`to_zod_export`, `to_valibot_export`).
- `vld-schemars`: OpenAPI 3.0 output under `SchemaSettings::openapi3()`.

### Changed

- Faster parsing: `serde_json::Value` input is borrowed, cached field schemas (regexes compiled once), default messages rendered without allocations per key.
- `vld-aide` reuses `vld-schemars` instead of duplicating the bridge.
- `vld-fake` generates values that satisfy far more schemas (patterns, all core formats, `$ref`, `multipleOf`, property counts) and seeds its randomness.
- `image` is built without its default `avif` encoder (only image headers are read).
- Packages ship only `src/`, README, LICENSE (and the changelog for `vld`).

### Fixed

- Strings: char-safe ISO date/time parsing (calendar-checked), strict IPv4/IPv6/CIDR via std, `url_strict` blocks loopback / private / link-local hosts, stricter email / phone / credit-card / emoji / cuid2, `base64url()` rejects `+` and `/`.
- Numbers: exact `ZInt` (no f64 round-trip), NaN never passes a bound, tolerant `multiple_of`, the strictest bound is reported, `f32::MAX` accepted.
- Collections / combinators: an inner error without issues no longer turns into success; union errors keep per-variant issues; maps reject duplicate keys; unique compares numbers by value; no panics in intersection / tuple / `schema!` / derive.
- Dates: one strict format for all backends; `time` datetime defaults rendered as RFC 3339.
- `ZPath::within` / `ZFile::within`: lexical containment with symlink resolution of existing ancestors; file reads are bounded.
- JSON Schema / OpenAPI: resolvable `$ref`s and nested components for `schema!` and derive, `required` omits optional fields, literals emit `const`, maps `prefixItems`, records `propertyNames` / min/maxProperties; `diff` sees changes inside optional fields and composites.
- Derive: generics, raw identifiers, serde `rename_all` / `rename(deserialize)` / `skip` / `default`; `syn` errors instead of panics.
- Web adapters: body limits → 413 everywhere (ntex overflow detection, salvo forms, rocket mount paths and form limits, warp chunked bodies, rama limit errors), actix `{id:\d+}` path patterns, percent-decoded path params and cookies, lossless query coercion, repeated keys as arrays.
- `vld-lapin`: `nack_decode` / `reject_decode` settle invalid messages instead of valid ones.
- `vld-sea`: chrono / uuid / json / decimal / bytes columns no longer validate as `null`.
- `vld-sqlx`: `VldInt` decodes INT4 / INT2 columns.
- `vld-utoipa`: annotations kept on refs / composites / arrays, `anyOf` and type arrays supported, exact u64 bounds, tuple `prefixItems`.
- `vld-schemars`: `$ref`s point at the generator's definitions path; the validator handles `$ref`, `additionalProperties`, `prefixItems`.
- `vld-ts`: valid JS output (escaped literals, quoted keys, `new RegExp`), descriptions and defaults kept.
- `vld-tonic`, `vld-tauri`, `vld-clap`, `vld-surrealdb`, `vld-leptos`, `vld-dioxus`: clean issue paths.
- `vld-config`: `VldConfigError::source()`; env example uses `try_parsing`.

### Security

- All 0.4.7 security fixes (see below).
- `vld-tower`: `Application/JSON` and `+json` bodies no longer bypass validation; JSON bodies are size-limited.
- Dependencies refreshed: 21 of 23 RustSec advisories cleared (two remaining, documented in `deny.toml`).

### Removed

- The `std` feature.
- Inherent `to_json_schema_inner()` / per-type `to_json_schema()` duplicates (the `JsonSchema` trait remains; `ZObject::to_json_schema` stays).
- `vld-http-common::extract_path_param_names` (unused).

## [0.4.7] - 2026-11-06
### Fixed

- **Security** `vld-poem`: `VldPath` read the query string instead of the matched route parameters, so `/users/42` failed validation and `/users/42?id=7` validated `id = 7` (client-controlled). It now uses `Request::path_params`.
- **Security** `vld`: a string longer than 100 bytes whose 97th byte falls inside a multi-byte character (e.g. Cyrillic, accented Latin) panicked while being stored as `received` in an error — any request body could crash the process. Truncation is now char-based.
- **Security** `vld`: `string().iso_date()` / `iso_time()` / `iso_datetime()` panicked on non-ASCII input (`"123é56789"`) and on `"2024-01-01T"`.
- **Security** `vld` (`time` backend): `datetime()` panicked when the UTC instant left the supported year range (`"9999-12-31T23:00:00-05:00"`); it is now a validation error. Requires `time >= 0.3.47`.
- **Security** `vld`: `string().url_strict()` never checked IPv6 hosts (`http://[::1]/`, `http://[fd00::1]/`, `http://[::ffff:127.0.0.1]/` passed) and allowed `0.0.0.0`, `localhost.` and `*.localhost`.
- **Security** `vld`: `path().within(base)` accepted `base/../outside` when the target did not exist (lexical `..` was not resolved).
- **Security** `vld-http-common`: with duplicate cookie names the last value won, allowing cookie tossing; the first occurrence now wins.

## [0.4.6] - 2026-11-05
### Fixed

- JSON Schema / OpenAPI: `.with_default(value)` now emits `"default": value` ([#8](https://github.com/s00d/vld/issues/8)). The `JsonSchema` impl for `ZDefault` requires the output type to be `serde::Serialize`
- `chrono`, `jiff`, `time` (human-readable) and `ipnet` optional deps now enable their `serde` features so date/network outputs keep working with `with_default` in schemas

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

