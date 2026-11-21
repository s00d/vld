//! `vld_data::sea1`: the shared SeaORM tests (`sea/`) compiled against
//! SeaORM 1 (the entities use `vld_data::sea1::sea_orm`); saves go through
//! SeaORM's `MockDatabase`.

use vld_data::sea1 as vld_sea;

#[allow(clippy::duplicate_mod)]
#[path = "sea/common.rs"]
mod common;
#[allow(clippy::duplicate_mod)]
#[path = "sea/model_values.rs"]
mod model_values;
#[allow(clippy::duplicate_mod)]
#[path = "sea/story_active.rs"]
mod story_active;
#[allow(clippy::duplicate_mod)]
#[path = "sea/story_feature_types.rs"]
mod story_feature_types;
#[allow(clippy::duplicate_mod)]
#[path = "sea/story_save.rs"]
mod story_save;
