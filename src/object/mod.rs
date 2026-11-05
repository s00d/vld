//! Dynamic objects ([`ZObject`]) and lenient-parsing results.

mod dynamic;
mod field;
mod lenient;

pub use crate::schema::DynSchema;
pub use dynamic::ZObject;
pub use lenient::{FieldResult, ParseResult};
