//! The schemars tests against `vld_openapi::schemars0`.

use vld_openapi::schemars0 as bridge;

#[allow(clippy::duplicate_mod)]
#[path = "schemars/model_differential.rs"]
mod model_differential;
#[allow(clippy::duplicate_mod)]
#[path = "schemars/story_aide_document.rs"]
mod story_aide_document;
#[allow(clippy::duplicate_mod)]
#[path = "schemars/story_introspection.rs"]
mod story_introspection;
#[allow(clippy::duplicate_mod)]
#[path = "schemars/story_schemars_to_vld.rs"]
mod story_schemars_to_vld;
#[allow(clippy::duplicate_mod)]
#[path = "schemars/story_vld_to_schemars.rs"]
mod story_vld_to_schemars;
