# vld-integration-common

[![crates.io](https://img.shields.io/crates/v/vld-integration-common.svg)](https://crates.io/crates/vld-integration-common)
[![docs.rs](https://docs.rs/vld-integration-common/badge.svg)](https://docs.rs/vld-integration-common)

Shared helpers for the [vld](https://crates.io/crates/vld) integration crates (`vld-dioxus`, `vld-leptos`, `vld-tauri`, `vld-clap`, `vld-sqlx`, `vld-diesel`, `vld-sea`, `vld-surrealdb`).

**Internal crate**: it has no stable public API of its own. Depend on the integration crate you need; each re-exports what it uses from here:

- `format_path` / `flatten_issues` — clean issue paths (`user.email`, `items[0].name`)
- `form` — `VldServerError`, `FieldError`, `check_field`, `validate_args!` (re-exported by `vld-dioxus` / `vld-leptos`)
- `db` — macros generating each database crate's error type, `Validated<S, T>` and the `VldText` / `VldInt` / `VldFloat` / `VldBool` column wrappers

## Documentation

- Guide: <https://s00d.github.io/vld/integrations/shared/integration-common>
- API: <https://docs.rs/vld-integration-common>

MSRV: 1.85, see [MSRV.md](https://github.com/s00d/vld/blob/main/MSRV.md).

## License

MIT
