//! `vld_data::bson2`: the shared bson tests (`bson/`) compiled against
//! bson 2.

use ::bson2 as bson;
use vld_data::bson2 as vld_bson;

#[allow(clippy::duplicate_mod)]
#[path = "bson/story_documents.rs"]
mod story_documents;
