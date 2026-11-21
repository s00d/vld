# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [Unreleased]

### Added

- `decimal()` scale and precision: `max_scale(n)` / `scale(n)` (digits after the point **as written**, trailing zeros count) and `max_precision(p)`; issues `too_big` / `invalid_length` with the new origins `scale` / `precision`; `ZDecimal::with_messages`; JSON Schema `x-maxScale` / `x-scale` / `x-maxPrecision`, honoured by `vld::fake`. Guide: [decimal](https://s00d.github.io/vld/schemas/special-types#scale-and-precision).
- `vld::money()` (feature `decimal`): `{amount, currency}` into `Money { amount: Decimal, currency: String }`, the amount's scale checked against the currency's ISO 4217 minor units (USD 2, JPY 0, BHD 3); `.amount(..)`, `.currencies(..)`. `Decimal` and `Money` implement `VldType`. Guide: [money](https://s00d.github.io/vld/schemas/special-types#money).
- Feature `geo` (no dependencies): `vld::latitude()`, `vld::longitude()`, `vld::geo_point()` (`{lat, lng}` or GeoJSON-order `[lng, lat]` into `GeoPoint`) and `vld::geojson()` (RFC 7946 geometries, `.types(..)` for Features / FeatureCollections, every issue at its exact path; new issue `ring_not_closed`). Guide: [geo](https://s00d.github.io/vld/schemas/special-types#geo).
- Number presets `vld::percent()` (`0..=100`) and `vld::ratio()` (`0..=1`).
- Unix timestamps on every date backend: `datetime().unix_seconds()` / `.unix_millis()` (integers, numeric strings with `.coerce()`), bounds and past / future as usual; JSON Schema `type: integer` with `format: unix-time` / `unix-time-ms` and the range. Guide: [Unix timestamps](https://s00d.github.io/vld/schemas/dates#unix-timestamps).
- `duration()` accepts full ISO 8601 durations: `P2W`, `P3DT4H30M`, `PT1.5H` (fraction on the last part); years and months are refused with `invalid_format` (`duration_calendar_unit`) as they have no fixed length.
- `vld::range(item)`: an ordered `{start, end}` pair (`.keys(..)` to rename) into `RangeInclusive<T>` for any ordered output (dates of every backend, numbers, decimals, durations); new issue `range_order` at the end field. Guide: [date ranges](https://s00d.github.io/vld/schemas/dates#date-ranges).
- `vld::crypto` (feature `crypto`): cryptocurrency wallet addresses with real encoding, checksum, version / prefix and length rules: Bitcoin (Base58Check, SegWit bech32, Taproot bech32m), Litecoin, Dogecoin, Bitcoin Cash (CashAddr and legacy), Ethereum / EVM (EIP-55), TRON, Solana, XRP, Cardano Shelley (CIP-19), Cosmos-SDK (any hrp), Polkadot / Kusama / Substrate (SS58, any prefix), TON (user-friendly and raw), Monero (standard, integrated, subaddress). `string().crypto_address(network)` / `crypto_address_any([...])`, typed `vld::crypto::address(network)` → `CryptoAddress { network, kind, testnet, normalized, payload }`, options `mainnet()` / `testnet()` / `kinds(..)` / `checksummed()`, and user templates `CryptoNetwork::custom(name)` from Base58 / Base58Check / bech32 / bech32m / hex / base64 encodings and SHA-256d / keccak / EIP-55 / SS58 / CRC16 / CRC32 checksums. Issue `invalid_crypto_address` with `network` and `reason`; JSON Schema `format: crypto-address` + `x-network` + `pattern`; `vld::fake` generates valid addresses. Guide: [crypto addresses](https://s00d.github.io/vld/schemas/crypto).
- `vld_app::env` (features `env`, `dotenv`): validated config from environment variables (`from_env`, `from_env_prefixed`, `from_vars`, `from_vars_prefixed`) and `.env` files via dotenvy (`from_dotenv`, `read_dotenv`); a prefix is stripped, `__` nests, values are coerced by the schema, and errors name the variable (`VldEnvError`). Guide: [env](https://s00d.github.io/vld/integrations/config-cli/env).
- `vld::schema::coerce_text`: the lossless text → JSON coercion (`"42"` → `42`, `"01234"` stays text) for query strings, forms, headers and env vars, moved from `vld_web::common::coerce_value` (now a re-export of it).
- `vld_web::async_graphql` (feature `async-graphql`, async-graphql 7, Rust 1.89): `VldValidator<S>` for `#[graphql(validator(custom = ...))]`, `validate::<S, T>` for resolvers and `vld_error`; issues in the error's `extensions.issues` with the HTTP issue shape. Guide: [async-graphql](https://s00d.github.io/vld/integrations/web/async-graphql).
- `vld_web::tungstenite` (feature `tungstenite`, tungstenite 0.26 – 0.30; serves tokio-tungstenite / async-tungstenite) and `vld_web::axum::ws` (feature `axum-ws`): `decode` / `encode` / `error_message` for WebSocket messages, the error frame carrying the HTTP 422 body. Guide: [WebSocket](https://s00d.github.io/vld/integrations/web/websocket).
- `vld_web::lambda` (feature `lambda`, `lambda_http` 1): `validate_json`, `validate_query`, `validate_path`, `error_response` for API Gateway / ALB requests, `validate_event` / `event_error` for raw `lambda_runtime` events. Guide: [AWS Lambda](https://s00d.github.io/vld/integrations/web/lambda).
- vld-data: `vld_data::bson2` / `bson3` (features `bson-2` / `bson-3`): `to_document`, `from_document`, `validate_document`, `validate_documents` for MongoDB documents, validated as relaxed Extended JSON ([MongoDB](https://s00d.github.io/vld/integrations/database/mongodb)); `vld_data::postgres_types` (feature `tokio-postgres`, postgres-types 0.2): `Validated<S, T>` JSON / JSONB and `VldText` / `VldInt` / `VldFloat` / `VldBool` columns implementing `ToSql` / `FromSql`, validated when built and when read ([tokio-postgres](https://s00d.github.io/vld/integrations/database/tokio-postgres)); `vld_data::clickhouse` (feature `clickhouse`, clickhouse 0.15, Rust 1.89): `insert_validated`, `fetch_all_validated`, `validate_rows` ([ClickHouse](https://s00d.github.io/vld/integrations/database/clickhouse)); `vld_data::rumqttc` (feature `rumqttc`, rumqttc 0.25): `publish`, `try_publish`, `decode_publish` ([MQTT](https://s00d.github.io/vld/integrations/messaging/mqtt)); `vld_data::nats` (feature `nats`, async-nats 0.53, Rust 1.89): `publish`, `request` (validated both ways), `respond`, `decode` ([NATS](https://s00d.github.io/vld/integrations/messaging/nats)); `vld_data::centrifuge` (feature `centrifuge`, centrifuge-client 0.1): `publish`, `client_publish`, `rpc`, `decode_publication`, `decode_rpc` ([Centrifugo](https://s00d.github.io/vld/integrations/messaging/centrifugo)).
- `vld_data::csv` (feature `csv`, csv 1): `rows` (an iterator of `Result<T, RowError>`) and `validate_all` (every row, or every failing row) read CSV records into vld types; the header row names the fields, cells are coerced like query strings, and `RowError` carries the record's line, its cells and the `VldError` or `csv::Error`. Guide: [CSV import](https://s00d.github.io/vld/integrations/database/csv).
- `vld_openapi::poem_openapi` (feature `poem-openapi`, poem-openapi 5): `impl_type!` implements poem-openapi `Type` / `ParseFromJSON` for a vld type (the OpenAPI component from `json_schema()`, nested types included, validation on parse); `VldJson<T>` is a request body answering 422 with every issue (the `vld_web` body); `json_schema_to_meta`, `validation_error`. Guide: [poem-openapi](https://s00d.github.io/vld/integrations/openapi/poem-openapi).
- `vld::llm` (feature `llm`, no dependencies): `T::llm_schema(provider)` / `schema.llm_schema(provider)` / `llm_schema(&json, provider)` prepare a JSON Schema for OpenAI strict structured outputs, Anthropic strict tools, Gemini `responseSchema` or plain draft 2020-12 (`Provider::Generic`): nested types in `$defs` (recursion as `$ref: "#"`), `oneOf` → `anyOf`, nullable `anyOf` → type arrays, every property required and optional ones nullable plus `additionalProperties: false` for OpenAI, keywords and formats a provider rejects moved into `description` as text, and provider limits (depth, property / enum counts, recursion, optional / union parameters) checked up front (`LlmSchemaError`). `parse_llm::<T>` / `parse_llm_with` / `extract_json` read the JSON out of a model reply (raw, fenced, inside prose); `VldError::llm_feedback()` / `llm_feedback_with(LlmFeedbackOptions)` / `llm_feedback_json()` render issues as `path: expected <rule>, got <value>` lines (issue limit, retry prompt, allowed-key hints, redaction kept). Guide: [LLM structured outputs](https://s00d.github.io/vld/openapi/llm).
- `vld_web::rmcp` (feature `rmcp`, rmcp 3.11, Rust 1.88): `VldParameters<T>`, `parse_args`, `input_schema::<T>()` and `VldToolError` (`IntoCallToolResult`): MCP tool arguments validated by vld, failures returned as `isError` results with the feedback text and structured issues (SEP-1303). Guide: [rmcp](https://s00d.github.io/vld/integrations/ai/rmcp).
- `vld_web::rig` (feature `rig`, rig-core 0.47, Rust 1.97.1): `VldToolHandler` + `VldTool<H>` implement rig's `Tool` with `parameters()` from vld (per provider) and `invalid_args` feedback on bad arguments; `parse_args` for hand-written tools. Guide: [rig](https://s00d.github.io/vld/integrations/ai/rig).
- Guide [loco-rs](https://s00d.github.io/vld/integrations/web/loco): `vld_web::axum` extractors in loco controllers, and rendering the rejection as loco's `ErrorDetail` (no new feature: loco handlers are axum handlers).
- String formats with real tables and checksums: `country()` / `country_alpha3()` (assigned ISO 3166-1 codes), `iban()` (registry length per country + mod-97), `bic()`, `isbn()` (ISBN-10 / ISBN-13 check digits), `ean()` (EAN-8 / UPC-A / EAN-13 / GTIN-14), `vin()` (North American check digit), `timezone()` (IANA name; checked against jiff's database with `jiff`, by shape without), `mime()`, `object_id()`, `regex_syntax()` (feature `regex`), each with its `_msg` twin, `StringValidation` variant, `with_messages` key, JSON Schema `format`, `vld::ts` check and `vld::fake` generator. Guide: [strings](https://s00d.github.io/vld/schemas/strings#identifiers).
- Password rules on `string()`: `require_lowercase()`, `require_uppercase()`, `require_digit()`, `require_symbol()`, `max_repeat(n)`, and `password_strength(score)` (feature `password-strength`, zxcvbn); one `invalid_string` issue per failed rule (`password_lowercase`, …, `password_repeat` with `max_repeat`, `password_strength` with `min_score`); JSON Schema `format: password` + `x-password`, honoured by `vld::ts` and `vld::fake`. Guide: [passwords](https://s00d.github.io/vld/schemas/strings#passwords).
- `VldSchema::redact()` (`ZRedact`): every issue of the wrapped schema, nested ones included, gets `received: "[redacted]"` (`vld::error::REDACTED`), so `Display`, `Debug`, serialized errors, translations and HTTP bodies never echo the value; custom messages quoting an input string are scrubbed too. Guide: [secrets in errors](https://s00d.github.io/vld/guides/security#secrets-in-errors).
- Feature `unicode`: `string().normalize_nfc()` / `normalize_nfkc()` transforms and the `single_script()` check (Unicode TS #39 mixed-script spoofs).
- Typed `vld::uuid()` → `uuid::Uuid` and `vld::url()` → `url::Url` (feature `string-advanced`, `.checks(..)` for extra string checks, JSON Schema `format: uuid` / `uri`); `uuid::Uuid` and `url::Url` implement `VldType`.

### Changed

- `string().currency_code()` checks the ISO 4217 table `money()` uses (active codes with minor units) instead of "three uppercase letters": withdrawn codes (`HRK`), metals and test codes (`XAU`, `XTS`) are now rejected.
- `string().cron()` parses the fields: values and ranges must be in range (minutes 0–59, hours 0–23, days 1–31, months 1–12 or `JAN`–`DEC`, weekdays 0–7 or `SUN`–`SAT`), steps are `1..=max`, `?` only in the day fields; `@daily` & co. are accepted. It used to accept any 5 or 6 fields of digits and `*/,-?`.
- The `jiff` feature enables jiff's time zone database (`tzdb-bundle-platform`, `tzdb-zoneinfo`) for `string().timezone()`.

## [0.8.0] - 2026-11-21

### Breaking changes

- **The 30 crates are merged into six**: `vld`, `vld-derive`, `vld-web`, `vld-openapi`, `vld-data`, `vld-app`, released together with one version. Every integration is a public module of its group crate behind a feature, with the items of the former crate (`VldJson`, its error type, `prelude`, …). The 28 crates they replace are no longer developed (each gets a final 0.7.1 pointing to its new home). Guide: [migration](https://s00d.github.io/vld/guides/migration).
- **No default features** in any crate: name what you use. Former defaults: `vld-ntex` → `ntex-4`; `vld-utoipa` → `utoipa-5`; `vld-schemars`, `vld-aide` → `schemars-0`; `vld-sqlx` → `sqlx-0_8` + `sqlite`; `vld-diesel` → `diesel` + `sqlite`; `vld-sea` → `sea-orm-1`; `vld-redis` → `redis-0`; `vld-lapin` → `lapin-2`; `vld-config` → `config-rs`.
- **Majors are additive**: each supported major of a dependency is its own feature and module (`ntex3` / `ntex4`, `utoipa5` / `utoipa6`, `schemars0` / `schemars1`, `sqlx08` / `sqlx09`, `sea1` / `sea2`, `redis0` / `redis1`, `lapin2` / `lapin3` / `lapin4`), and any set can be enabled together; enabling two used to be a compile error.
- **Macros moved from the crate root to their module**: `vld::ts::{impl_to_zod, impl_to_zod4, impl_to_valibot, impl_to_ts_type, impl_to_openapi}!`, `vld::fake::impl_fake!`, `vld::form::validate_args!`, `vld_openapi::utoipa5::impl_to_schema!` / `utoipa6::impl_to_schema!`, `vld_openapi::schemars0::{impl_json_schema, impl_vld_parse}!` / `schemars1::…`, `vld_web::tonic::impl_validate!`, `vld_data::sea1::impl_vld_before_save!` / `sea2::…`, `vld_data::surrealdb::validate_fields!`.
- Renamed features: vld-redis `aio` / `aio-tokio` → `redis-aio` / `redis-aio-tokio`; vld-ntex `compress` → `ntex-compress`. vld-http-common's `conformance` feature is removed (the suite is test-only in `vld-web`).
- `vld::form` (former vld-leptos / vld-dioxus) has no `prelude`; import its items directly.
- vld-lapin: `VldLapinError` is `#[non_exhaustive]` and has a new variant `NotSettled`.

### Migration

| 0.7 crate | 0.8 crate + feature | Path |
|---|---|---|
| `vld-http-common` | `vld-web` | `vld_web::common` |
| `vld-axum`, `vld-actix`, `vld-poem`, `vld-rocket`, `vld-salvo`, `vld-warp`, `vld-rama`, `vld-tower`, `vld-tonic` | `vld-web` + `<name>` | `vld_web::<name>` |
| `vld-ntex` | `vld-web` + `ntex-3` / `ntex-4` (+ `ntex-compress`) | `vld_web::ntex3` / `vld_web::ntex4` |
| `vld-utoipa` | `vld-openapi` + `utoipa-5` / `utoipa-6` | `vld_openapi::utoipa5` / `utoipa6` |
| `vld-schemars`, `vld-aide` | `vld-openapi` + `schemars-0` / `schemars-1` | `vld_openapi::schemars0` / `schemars1`; validator: `vld_openapi::validator` (always on) |
| `vld-ts` | `vld` + `ts` | `vld::ts` |
| `vld-fake` | `vld` + `fake` | `vld::fake` |
| `vld-leptos`, `vld-dioxus` | `vld` | `vld::form` |
| `vld-sqlx` | `vld-data` + `sqlx-0_8` / `sqlx-0_9` + `postgres` / `mysql` / `sqlite` | `vld_data::sqlx08` / `sqlx09` |
| `vld-diesel` | `vld-data` + `diesel` + `postgres` / `mysql` / `sqlite` | `vld_data::diesel` |
| `vld-sea` | `vld-data` + `sea-orm-1` / `sea-orm-2` (+ `with-*`, `postgres-array`) | `vld_data::sea1` / `sea2` |
| `vld-surrealdb` | `vld-data` | `vld_data::surrealdb` |
| `vld-redis` | `vld-data` + `redis-0` / `redis-1` (+ `redis-aio`, `redis-aio-tokio`) | `vld_data::redis0` / `redis1` |
| `vld-lapin` | `vld-data` + `lapin-2` / `lapin-3` / `lapin-4` | `vld_data::lapin2` / `lapin3` / `lapin4` |
| `vld-config` | `vld-app` + `config-rs` / `figment` | `vld_app::config` |
| `vld-clap` | `vld-app` + `clap` | `vld_app::clap` |
| `vld-tauri` | `vld-app` | `vld_app::tauri` |
| `vld-integration-common` | `vld` | `vld::form`, `vld::error::{format_path, flatten_issues}` |

Using both SQLx majors in one process: each maps SQLite `":memory:"` to the same shared-cache name (`sqlx-in-memory-0`), so give in-memory databases distinct names.

### Added

- `#[derive(Validate)]`: `T::validate_field(name, value)` checks one field by its deserialize name without the rest of the object.
- `#[derive(Validate)]` honours `skip_serializing` / `skip_serializing_if`: `validate()` accepts a serialized form without those keys.
- `[T; N]` implements `VldType` (an array of exactly `N` items).
- Numbers: `one_of(&[..])` (`invalid_enum_value`, JSON Schema `enum`), keeping the numeric output.
- Strings: `min_bytes` / `max_bytes` (UTF-8 bytes, origin `bytes`) and the `truncate(n)` transform.

### Fixed

- A type-range failure (`u8()` above 255, `f32()` overflow) now uses the `with_messages` text for `too_small` / `too_big`.
- `number()` checks whole numbers exactly: `multiple_of` and bounds were judged in `f64` with a tolerance and accepted e.g. `2^63` as a multiple of 3.
- Two `#[vld(...)]` attributes on one field were silently reduced to the first; now a compile error.
- vld-lapin on lapin 3 / 4: an ack / nack / reject that sends nothing (delivery already settled, channel closed) returned `Ok(false)`, which the helpers dropped, so `ack_decode` reported success for an unacked message. They now return `VldLapinError::NotSettled`.
- vld-sea `with-time`: time fractions are written in 3, 6 or 9 digits like chrono (`12:30:05.500`, was `12:30:05.5`), so `time` and chrono columns give the same text.
- `number().multiple_of(..)` is exact for whole numbers beyond `i128` too.
- `array().unique()` compares whole floats with integers by exact value at any magnitude (`9.3e18` and `9300000000000000000` are duplicates; floats at or above `9.2e18` were never equal to an integer).
- Numbers in `contains`, `literal` and discriminators compare by exact value: `9007199254740993` no longer equals `9007199254740992.0` (they were compared as `f64`).
- vld-openapi validator: `const`, `enum` and `uniqueItems` compare numbers by value as JSON Schema requires (`2` equals `2.0`; it compared them as different).
- `schema!`: a serde `rename` containing escapes (`rename = "a\"b"`) now gives the key serde uses (`a"b`); the escape text was kept verbatim.
- A `union` with a branch that accepts a missing key (`optional()`, `with_default`, …) is no longer listed in the object's JSON Schema `required`.
- vld-schemars `to_openapi3`: a null-only schema (`{"type": ["null"]}`, an `anyOf` of null branches) became `{"nullable": true}`, which allows any value; it now stays null-only.
- vld-schemars validator: `multipleOf` with a fractional divisor accepted every value once the quotient passed ~1e9 (`100000000.35` for `0.1`); the tolerance is now 16 ulps.
- vld-fake: field-name templates (e.g. `price`) replaced records, strict empty objects and objects with `propertyNames` / property counts, so the fake failed its own schema; they apply to free-form objects only.
- vld-fake: arrays of 2-tuples could stop short of `minItems` when distinct keys ran out.
- vld-ts Zod 4: `date-time` used `z.iso.datetime`, which accepts `HH:MM` and 10+ fraction digits and rejects lowercase `t` / `z`, unlike vld; it now uses the Zod 3 regex and calendar check, and `time` gets a seconds regex.

## [0.7.0] - 2026-11-07

### Breaking changes

- vld-tauri: `TauriIssue` has two new public fields, `code` and `params`; code that builds it with a struct literal must set them (deserializing old JSON still works, both default to empty).

### Added

- `#[derive(Validate)]`: a field without `#[vld(...)]` gets its type's default schema through the new `vld::schema::VldType` trait (`String`, `bool`, numbers, `Option<T>`, `Vec<T>`, `HashSet<T>`, `HashMap/BTreeMap<String, T>`, `serde_json::Value`, derived and `schema!` types as nested `$ref`s). Tuple and newtype enum variants infer the same way.
- `#[vld(dynamic)]` on a derived struct or enum rebuilds its schemas on every parse, for limits read at runtime (non-generic types cache them once per process).
- `vld::de`: `#[serde(deserialize_with = "vld::de::parse")]` for `VldParse` fields and `vld::deserialize_fn!(fn name -> T = <schema>)` to validate while serde deserializes.
- Numbers: `clamp()` moves values outside the inclusive bounds onto them instead of failing; sized integers saturate to their type, `f32` to its finite range.
- `VldSchema::blank_as(v)`: `with_default(v)` that also maps a blank string (`""`, whitespace).
- `ZString::coerce_bools(on, off)`: coercion that writes booleans as e.g. `"1"` / `"0"`.
- vld-tauri: `TauriIssue` carries `code` and `params` for frontend translation (vue-i18n & co.).
- vld-ts: Zod 4 output (`to_zod4`, `impl_to_zod4!`, `zod4_export`) and plain TypeScript types (`to_ts_type`, `impl_to_ts_type!`, `ts_type_export`).

## [0.6.1] - 2026-11-07

### Added

- `#[derive(Validate)]` honours `#[serde(deny_unknown_fields)]` on structs and enums: unknown keys fail with `unrecognized_field` (the tag / `content` keys of tagged enums are allowed), and the JSON Schema gets `additionalProperties: false`. On a unit variant of an internally tagged enum vld rejects extra keys, which serde lets through.
- `VldSchema::try_transform(|v| -> Result<U, E>)`: a fallible transform whose `Err` (`E: Display`) becomes a validation issue (`Custom { code: "transform" }`).

## [0.6.0] - 2026-11-07

### Breaking changes

- **Regex**: `regex-lite` is no longer public. `string().regex(Regex)` / `regex_msg` and the `vld::regex_lite` re-export are removed; use `string().pattern("...")` / `pattern_msg`, or `vld::rules::Pattern::new(..)` to handle an invalid pattern. Both need the `regex` feature, as `regex()` did.
- **optional / nullable / nullish** now differ: `nullable()` object fields are required (absent → `missing_field`, `null` → `None`) and listed in JSON Schema `required`; `optional()` no longer adds `null` to its JSON Schema (parsing still accepts `null`, which serde writes for `None`). TypeScript output follows: `x?: T`, `x: T | null`, `x?: T | null`.
- **Unions**: `Either` / `Either3` serialize untagged, so a union inside an object round-trips as the bare value instead of `{"Left": ...}`; unions forward `output_json` to the matching variant.
- **Removed** everything deprecated in 0.5: the `serialize` / `deserialize` features, `ZFloat::finite`, `ZObject::field_schema` / `deep_partial`, `vld-actix::VldJsonError`, `vld-diesel` / `vld-sqlx::validate_update`, `vld-surrealdb::validate_record`, `vld-http-common::{ValidationIssueWithCode, format_issues_with_code, format_json_parse_error, format_utf8_error}`, `vld-tower` / `vld-rama::try_validated`, `vld-tauri::{validate_event, validate_state, validate_plugin_config, validate_channel_message}`, `impl_to_lapin!`, `impl_to_redis!`, `impl_to_redis_async!`, `vld-utoipa::{impl_into_params!, impl_to_schema_query!, impl_to_schema_path!}`.
- `DynSchema` has a new required method `dyn_parse_missing` (implemented for every `VldSchema`; only hand-written impls need it).

### Migration from 0.5

See the [migration guide](https://s00d.github.io/vld/guides/migration).

### Added

- **Custom rules** (`vld::rules`): the `Rule<T>` trait (key, check, optional message and JSON Schema keywords), `rules::from_fn`, `ZString::rule` / `rule_msg` (keeps the string builder) and `VldSchema::rule` / `rule_msg` on any schema. A failing rule reports `Custom { code: <key> }`, so it can be translated by key.
- `#[derive(Validate)]` on enums, following serde: externally tagged, `tag`, `tag` + `content`, `untagged`; unit, newtype, tuple and struct variants; variant `rename` / `alias` / `skip` / `other` / `rename_all` and enum `rename_all` / `rename_all_fields`. The output is the enum itself; the JSON Schema is `oneOf` with an OpenAPI `discriminator` (`anyOf` when untagged).
- `#[derive(Validate)]` supports `#[serde(flatten)]` on fields: parsed from the same object, JSON Schema properties merged.
- `VldSchema::parse_missing`: what an absent object key parses to (wrappers forward it).
- `vld::Schema`: `VldSchema` plus the JSON Schema traits under `openapi`, for `fn f() -> impl vld::Schema<Output = T>`.
- `ValidationIssue::path_string()` (`.lab.fund[0].vigor`).
- Inherent `vld_parse_value` on `#[derive(Validate)]` / `schema!` types (no `VldParse` import needed).
- `Either::into_inner` when both sides have the same type.

### Fixed

- `vld-schemars`: `pattern` and `patternProperties` regexes are compiled once and cached instead of on every validated value.

## [0.5.1] - 2026-11-06

### Changed

- All crates use Rust edition 2024 (MSRV unchanged: 1.85).
- Dependencies used only by tests are no longer normal dependencies (`serde` / `serde_json` in several integration crates); unused ones are removed (`hyper`, `http-body-util` in `vld-axum`, `serde` in `vld-config`, `tokio` in `vld-tonic`, `hyper` / `hyper-util` in `vld-tower`).
- `vld-fake`: password generation no longer uses `unsafe`; every crate forbids `unsafe_code`.
- Every public item has rustdoc documentation (`missing_docs` is on in every library crate).

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


## [0.1.3] - 2026-03-19



### Added


- Add health check endpoint and response schemas

- Generate json_schema() for derive(Validate), enabling utoipa integration

