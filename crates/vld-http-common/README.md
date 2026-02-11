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
