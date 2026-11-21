//! `vld_data::bson3`: the shared bson tests (`bson/`) compiled against
//! bson 3.

use ::bson3 as bson;
use vld_data::bson3 as vld_bson;

#[allow(clippy::duplicate_mod)]
#[path = "bson/story_documents.rs"]
mod story_documents;
