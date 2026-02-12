# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2026-02-11

### Added

- **`vld-fake`** — new crate: generate fake / test data from `vld` JSON Schemas.
  - Typed API: `impl_fake!(User)` → `User::fake()`, `User::fake_many(n)`, `User::fake_seeded(seed)`, `User::try_fake()`.
  - Low-level API: `fake_value()`, `fake_json()`, `fake_many()`, `fake_parsed::<T>()`, `fake_value_seeded()`.
  - 20+ string formats: email, uuid, url, ipv4, ipv6, hostname, date, time, date-time, base64, cuid2, ulid, nanoid, emoji, phone, credit-card, mac-address, hex-color, semver, slug.
  - Smart field-name heuristics (~60 patterns): `name` → realistic full name, `email` → first.last@domain, `phone` → +1 (555) 123-4567, `city` → real city name, `id` → UUID, etc.
  - Object templates for empty `{"type":"object"}` schemas: address, geo, person, profile, company, product, money, image, config, metadata, dimensions.
  - Number heuristics: latitude/longitude, price, rating, temperature, percentage, weight, speed, distance.
  - Built-in dictionaries: first/last names, cities, countries, states, streets, companies, departments, job titles, adjectives, product nouns, categories, tags, industries, email domains, TLDs, colors, currencies, locales, timezones, emojis, lorem words.
  - Reproducible generation via seeded RNG (`StdRng`).
  - `uniqueItems` support for arrays.
  - Luhn-valid credit card numbers, valid ISBN-13.
  - `FakeGen<R: Rng>` for custom RNG.
- **`vld-salvo`** — new crate: [Salvo](https://salvo.rs/) web framework integration.
  - Extractor types: `VldJson<T>`, `VldQuery<T>`, `VldPath<T>`, `VldForm<T>`, `VldHeaders<T>`, `VldCookie<T>`.
  - Implements Salvo's `Extractible` trait — used directly as `#[handler]` parameter types.
  - `VldSalvoError` with `salvo::Writer` impl for 422 JSON error responses.
  - `Deref`/`DerefMut` on all wrappers for direct field access.
- **`vld-tauri`** — new crate: [Tauri](https://tauri.app/) IPC validation.
  - `validate::<T>(payload)` for explicit validation of IPC command arguments.
  - `VldPayload<T>` / `VldEvent<T>` — auto-validating `Deserialize` wrappers.
  - Specialized functions: `validate_event`, `validate_state`, `validate_plugin_config`, `validate_channel_message`.
  - `VldTauriError` — serializable error type for frontend.
  - Zero dependency on `tauri` crate.
- **`vld-http-common`** — shared HTTP helpers extracted from web integration crates.
  - `coerce_value()`, `parse_query_string()`, `cookies_to_json()`, `url_decode()`.
  - `vld::schema!` defined error response types: `ErrorBody`, `ErrorWithMessage`, `ValidationErrorBody`, `ValidationIssue`, `ValidationIssueWithCode`.
  - Error formatting helpers: `format_vld_error()`, `format_json_parse_error()`, `format_utf8_error()`, `format_payload_too_large()`, `format_generic_error()`.
- **`vld-warp`**: path parameter validation — `vld_param()`, `vld_path()`, `validate_path_params()`.
- **`string().coerce()`**: coerce numbers/booleans to string.
- **`ZDate` / `ZDateTime`** types with `chrono` parsing (optional `chrono` feature).
- **`ZMessage<T>`** combinator for custom error messages.
- **`ZObject::field_optional()`** / **`ZObject::when()`** for conditional validation.
- **`MessageResolver`** trait for i18n of error messages.
- **`SchemaDiff` / `diff_schemas`** for comparing JSON Schemas (optional `diff` feature).
- **`impl_default!`** macro for `Default` implementations on `schema!` structs.
- **`#[serde(rename)]` / `rename_all`** support in derive macro.
- **Property-based tests** with `proptest`.
- **`--no-default-features` CI tests**.
- Root README describing all workspace crates.
- Badges on all README files.

### Changed

- All web integration crates (`vld-axum`, `vld-actix`, `vld-poem`, `vld-rocket`, `vld-warp`, `vld-tower`) now use `vld-http-common` for shared helpers instead of duplicating code.
- All inline `serde_json::json!` error responses replaced with `vld` schema-based types from `vld-http-common`.
- All example responses in web crates use `vld::schema!` structs instead of raw `serde_json::json!`.
- `vld-clap`: redesigned to use `#[derive(Validate)]` directly on `clap::Parser` structs.
- All sub-crates moved into `crates/` subdirectory.
- Tests moved to `tests/` folders in all crates.
- CI: removed Rust 1.70 matrix entry (Cargo.lock v4 incompatibility).

### Fixed

- `VldInput` for `Path` gated behind `std` feature.
- `parse_result_save_to_file` test gated behind `serialize + std`.
- `unused import` warnings with `--no-default-features`.
- crates.io publishing: batched publishing guide for rate limits.

## [0.1.0] - 2026-02-11

### Added

- Core validation trait `VldSchema` with `parse()`, `parse_value()`, `validate()`, `is_valid()`.
- **Primitives**: `string()`, `number()`, `boolean()`, `literal()`, `enumeration()`, `any()`.
- **String formats**: email, URL, UUID, IPv4, IPv6, Base64, ISO date/time/datetime, hostname, CUID2, ULID, Nano ID, emoji. All without regex by default.
- **Number checks**: min, max, gt, lt, positive, negative, non_negative, non_positive, finite, multiple_of, safe, int.
- **Collections**: `array()`, `record()`, `map()`, `set()`, tuple schemas (up to 6 elements).
- **Modifiers**: `optional()`, `nullable()`, `nullish()`, `with_default()`, `catch()`.
- **Combinators**: `refine()`, `super_refine()`, `transform()`, `pipe()`, `preprocess()`, `describe()`, `.or()`, `.and()`.
- **Unions**: `union()`, `union3()`, `union!` macro (2–6 schemas), `discriminated_union()`, `intersection()`.
- **Recursive schemas**: `lazy()` for self-referencing data structures.
- **Dynamic objects**: `ZObject` with `strict()`, `strip()`, `passthrough()`, `pick()`, `omit()`, `extend()`, `merge()`, `partial()`, `required()`, `catchall()`, `keyof()`.
- **Custom schemas**: `vld::custom(|v| ...)` for arbitrary validation logic.
- **Multiple input sources**: parse from `&str`, `String`, `&[u8]`, `Path`, `PathBuf`, `serde_json::Value`.
- **Macros**: `schema!`, `impl_validate_fields!`, `schema_validated!`, `impl_rules!`.
- **Derive macro**: `#[derive(Validate)]` with `#[vld(...)]` attributes (optional `derive` feature).
- **Lenient parsing**: `parse_lenient()` returns `ParseResult<T>` with per-field diagnostics.
- **Error formatting**: `prettify_error()`, `flatten_error()`, `treeify_error()`.
- **Custom error messages**: `_msg` variants, `type_error()`, `int_error()`, `with_messages()`.
- **Coercion**: `coerce()` on `ZNumber` and `ZBoolean`.
- **JSON Schema / OpenAPI 3.1**: `JsonSchema` trait, `json_schema()`, `to_openapi_document()`, `to_openapi_document_multi()` (optional `openapi` feature).
- **Feature flags**: `serialize`, `deserialize`, `openapi`, `regex`, `derive` — all disabled by default.
- **Workspace**: playground example as a separate crate with all features enabled.
- **Benchmarks**: criterion-based validation benchmarks.
- **CI**: GitHub Actions workflow for testing, clippy, and formatting.
