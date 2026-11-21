//! `vld_data::sqlx09`: the shared SQLx tests (`sqlx/`) compiled against
//! SQLx 0.9, through an in-memory SQLite pool.

use ::sqlx09 as sqlx;
use vld_data::sqlx09 as vld_sqlx;

/// Tags this major's in-memory databases (`common::memory_pool`).
const MAJOR: &str = "sqlx09";

#[allow(clippy::duplicate_mod)]
#[path = "sqlx/common.rs"]
mod common;
#[allow(clippy::duplicate_mod)]
#[path = "sqlx/model_columns.rs"]
mod model_columns;
#[allow(clippy::duplicate_mod)]
#[path = "sqlx/story_columns.rs"]
mod story_columns;
#[allow(clippy::duplicate_mod)]
#[path = "sqlx/story_rows.rs"]
mod story_rows;
