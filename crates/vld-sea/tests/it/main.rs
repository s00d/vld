//! `vld-sea` behaviour tests: one test binary for both SeaORM majors (the
//! entities use `vld_sea::sea_orm`, the selected one). Saves go through
//! SeaORM's `MockDatabase` (see `TESTING.md` at the repository root).

mod common;

mod model_values;
mod story_active;
mod story_feature_types;
mod story_save;
