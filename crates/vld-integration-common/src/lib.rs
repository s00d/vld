//! # vld-integration-common — shared helpers for `vld` integration crates
//!
//! **Internal crate** with no stable API of its own: depend on the
//! integration crate you need (`vld-sqlx`, `vld-diesel`, `vld-sea`,
//! `vld-surrealdb`), which re-exports what it uses from here.
//!
//! - [`db`] — hidden macros that generate each database crate's error type,
//!   `Validated<S, T>` and the `VldText` / `VldInt` / `VldFloat` / `VldBool`
//!   column wrappers inside that crate (so it can implement its own database
//!   traits without orphan-rule conflicts).
//!
//! Clean issue paths and form errors moved to `vld`:
//! `vld::error::{format_path, flatten_issues}` and `vld::form`.
//!
//! Guide: <https://s00d.github.io/vld/integrations/shared/integration-common>
#![warn(missing_docs)]

pub mod db;
