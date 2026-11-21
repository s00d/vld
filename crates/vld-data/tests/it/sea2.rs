//! `vld_data::sea2`: the shared SeaORM tests (`sea/`) compiled against
//! SeaORM 2 (the entities use `vld_data::sea2::sea_orm`); saves go through
//! SeaORM's `MockDatabase`.

use vld_data::sea2 as vld_sea;

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
#[path = "sea/story_native_enums.rs"]
mod story_native_enums;
#[allow(clippy::duplicate_mod)]
#[path = "sea/story_save.rs"]
mod story_save;
