//! Core `vld` integration tests — one test binary, one module per area.
//!
//! Feature-gated areas are `cfg`'d modules, so the same binary runs under
//! every feature combination (`--no-default-features` included).
//! `tests/new_types.rs` stays a separate binary.

#[macro_use]
mod common;

mod bool_enum_literal;
mod collections;
mod combinators;
#[cfg(any(feature = "chrono", feature = "jiff", feature = "time"))]
mod dates;
#[cfg(feature = "diff")]
mod diff;
mod errors;
mod file_path_bytes_net;
mod input;
mod int;
#[cfg(feature = "openapi")]
mod json_schema;
mod macros;
mod modifiers;
mod number;
mod object;
mod regressions;
mod string;
mod proptests;
