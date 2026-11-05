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
| `parse_form_body` | Parse an urlencoded body (422 on invalid UTF-8) |
| `parse_json_body` | Parse a JSON body (422 on malformed JSON) |
| `cookies_to_json` | Parse `Cookie` header (unquoted, percent-decoded values) |
| `headers_to_json` | Header pairs → JSON (`X-Id` → `x_id`, repeated joined with `", "`, lossy UTF-8) |
| `path_params_to_json` | Decoded path params → JSON |
| `is_json_content_type` | `application/json` / `application/*+json`, case-insensitive |
| `VldHttpError` | Shared error: status (422 / 413) + JSON body |
| `format_issues` | Format `VldError` issues as JSON array |
| `format_vld_error` | Format `VldError` as full JSON error body |
| `format_issues_with_code` | Format issues with `code` field |
| `url_decode` / `percent_decode` | UTF-8-correct percent decoding (form variant maps `+` to space) |
| `DEFAULT_BODY_LIMIT` | 2 MiB default body limit |

## OpenAPI error responses

Framework crates return validation errors using helpers from this crate. The
`ValidationErrorBody` shape (via `format_vld_error`) can be registered in OpenAPI as a `422`
response with `impl_to_schema!(ValidationErrorBody)` from
[vld-utoipa](../vld-utoipa/README.md) (enable `openapi` on `vld`).

## License

MIT
