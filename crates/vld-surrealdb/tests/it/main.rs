//! `vld-surrealdb` behaviour tests: one test binary, modules are stories and
//! models (see `TESTING.md` at the repository root). The crate has no SDK
//! dependency, so documents travel as `serde_json` values, as the SDK sends
//! and returns them.

mod common;

mod model_documents;
mod story_columns;
mod story_documents;
mod story_merge;
