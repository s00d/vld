[![Crates.io](https://img.shields.io/crates/v/vld-http-common?style=for-the-badge)](https://crates.io/crates/vld-http-common)
[![docs.rs](https://img.shields.io/docsrs/vld-http-common?style=for-the-badge)](https://docs.rs/vld-http-common)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=for-the-badge)](https://github.com/s00d/vld)
[![GitHub issues](https://img.shields.io/badge/github-issues-orange?style=for-the-badge)](https://github.com/s00d/vld/issues)
[![GitHub stars](https://img.shields.io/badge/github-stars-yellow?style=for-the-badge)](https://github.com/s00d/vld/stargazers)

# vld-http-common

Shared HTTP utility functions for `vld` web-framework integration crates.

This crate is **internal** — not intended for direct use. Import helpers via
the framework-specific crate (`vld-actix`, `vld-axum`, `vld-ntex`, `vld-poem`,
`vld-rama`, `vld-rocket`, `vld-salvo`, `vld-tower`, `vld-warp`, `vld-tonic`)
instead.

## Provided helpers

| Function | Description |
|---|---|
| `coerce_value` | Lossless string → JSON coercion (canonical numbers, `true`/`false`; everything else stays a string) |
| `parse_query_string` | Parse a query string / form body to `serde_json::Map` (repeated keys and `a[]` → arrays) |
| `query_string_to_json` | Same, as `serde_json::Value` |
| `parse_form_body` | Parse an urlencoded body (422 on invalid UTF-8 or more than `MAX_FORM_FIELDS` = 1000 fields) |
| `parse_json_body` | Parse a JSON body (422 on malformed JSON) |
| `cookies_to_json` | Parse a `Cookie` header (unquoted, percent-decoded values; first occurrence of a name wins) |
| `cookie_headers_to_json` | All `Cookie` header values of a request, joined with `"; "` (HTTP/2 sends one per cookie) |
| `headers_to_json` | Header pairs → JSON (`X-Id` → `x_id`, names containing `_` skipped, repeated joined with `", "`, lossy UTF-8) |
| `path_params_to_json` | Decoded path params → JSON |
| `is_json_content_type` | `application/json` / `application/<x>+json` (non-empty `<x>`), case-insensitive |
| `require_json_content_type` / `require_form_content_type` | 415 unless JSON / `application/x-www-form-urlencoded` |
| `VldHttpError` | Shared error: status (422 / 413 / 415) + JSON body |
| `VldBodyLimit` | Body limit registered as app/request data (actix, ntex, poem) |
| `format_issues` | Format `VldError` issues (`path`, `message`, `code`) |
| `format_vld_error` | Format `VldError` as the full 422 body |
| `url_decode` / `percent_decode` | UTF-8-correct percent decoding (form variant maps `+` to space) |
| `DEFAULT_BODY_LIMIT` | 2 MiB default body limit |

Deprecated in 0.5: `ValidationIssueWithCode` / `format_issues_with_code` (aliases of
`ValidationIssue` / `format_issues`), `format_json_parse_error`, `format_utf8_error` (unused by
the adapters).

## Error bodies

Every adapter answers with the same JSON bodies:

| Status | When | Body |
|---|---|---|
| 422 | schema validation failed; malformed JSON, non-UTF-8 or oversized (> 1000 fields) form | `{"error": "Validation failed", "issues": [...]}` |
| 413 | body over the limit | `{"error": "Payload too large"}` |
| 415 | JSON extractor without a JSON `Content-Type`, form extractor without `application/x-www-form-urlencoded` | `{"error": "Unsupported Media Type", "message": "Expected Content-Type: ..."}` |

```json
{
  "error": "Validation failed",
  "issues": [
    { "path": ".name", "message": "String must be at least 2 characters", "code": "too_small" },
    { "path": ".items[0].qty", "message": "Number must be at least 1", "code": "too_small" }
  ]
}
```

`path` is `""` for the root (e.g. malformed JSON: `{"path": "", "code": "parse_error", ...}`),
`.field` for fields and `[i]` for array items; `code` is `IssueCode::key()`.

## Body limits

JSON and form bodies are limited to `DEFAULT_BODY_LIMIT` (2 MiB) by every adapter unless
configured:

| Adapter | Configure |
|---|---|
| vld-actix | `App::new().app_data(VldBodyLimit(n))` (`JsonConfig` / `PayloadConfig` do not apply) |
| vld-axum | `.layer(DefaultBodyLimit::max(n))` |
| vld-ntex | app state `VldBodyLimit(n)` (`JsonConfig` / `PayloadConfig` do not apply) |
| vld-poem | `.data(VldBodyLimit(n))` |
| vld-rama | `vld_rama::VldBodyLimit(n)` request extension; `ValidateJsonLayer::with_limit(n)` |
| vld-rocket | `limits.json` / `limits.form` (Rocket's built-in 1 MiB / 32 KiB count as unset) |
| vld-salvo | `secure_max_size` (`SecureMaxSize`, …; Salvo's built-in 64 KiB counts as unset) |
| vld-tower | `ValidateJsonLayer::with_limit(n)` |
| vld-warp | `vld_json_limit(n)` / `vld_form_limit(n)` |

Form bodies additionally have at most `MAX_FORM_FIELDS` (1000) fields.

## OpenAPI error responses

Framework crates return validation errors using helpers from this crate. The
`ValidationErrorBody` shape (via `format_vld_error`) can be registered in OpenAPI as a `422`
response with `impl_to_schema!(ValidationErrorBody)` from
[vld-utoipa](../vld-utoipa/README.md) (enable `openapi` on `vld`).

## License

MIT
