//! The utoipa tests against `vld_openapi::utoipa5`.

use vld_openapi::utoipa5 as bridge;

#[allow(clippy::duplicate_mod)]
#[path = "utoipa/corpus_conversion.rs"]
mod corpus_conversion;
#[allow(clippy::duplicate_mod)]
#[path = "utoipa/model_conversion.rs"]
mod model_conversion;
#[allow(clippy::duplicate_mod)]
#[path = "utoipa/story_document.rs"]
mod story_document;
