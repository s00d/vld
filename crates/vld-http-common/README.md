[![Crates.io](https://img.shields.io/crates/v/vld-http-common?style=for-the-badge)](https://crates.io/crates/vld-http-common)
[![docs.rs](https://img.shields.io/docsrs/vld-http-common?style=for-the-badge)](https://docs.rs/vld-http-common)
[![License](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](https://github.com/s00d/vld/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue?style=for-the-badge)](https://github.com/s00d/vld)
[![GitHub issues](https://img.shields.io/badge/github-issues-orange?style=for-the-badge)](https://github.com/s00d/vld/issues)
[![GitHub stars](https://img.shields.io/badge/github-stars-yellow?style=for-the-badge)](https://github.com/s00d/vld/stargazers)

# vld-http-common

Shared HTTP utility functions for `vld` web-framework integration crates.

This crate is **internal** — not intended for direct use. Import helpers via
the framework-specific crate (`vld-axum`, `vld-actix`, `vld-rocket`,
`vld-poem`, `vld-warp`) instead.

## Provided helpers

| Function | Description |
|---|---|
| `coerce_value` | Coerce a string to a typed JSON value |
| `parse_query_string` | Parse URL query string to `serde_json::Map` |
| `query_string_to_json` | Parse URL query string to `serde_json::Value` |
| `cookies_to_json` | Parse `Cookie` header to JSON object |
| `format_issues` | Format `VldError` issues as JSON array |
| `format_vld_error` | Format `VldError` as full JSON error body |
| `format_issues_with_code` | Format issues with `code` field |
| `url_decode` | Minimal percent-decode |
| `extract_path_param_names` | Extract `{param}` names from route pattern |

## License

MIT
