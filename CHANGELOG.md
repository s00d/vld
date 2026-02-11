# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
