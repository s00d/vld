//! `vld-diesel` behaviour tests: one test binary, modules are stories and
//! models through an in-memory SQLite connection (see `TESTING.md` at the
//! repository root).

// diesel's QueryableByName derive expands to `Self { id: id, .. }`, which clippy
// >= 1.99 flags as redundant_field_names; an item attribute does not reach the impl.
#![allow(clippy::redundant_field_names)]

mod common;

mod model_narrow;
mod story_columns;
mod story_rows;
