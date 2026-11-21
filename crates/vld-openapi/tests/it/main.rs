//! vld-openapi behaviour tests: models, stories and corpora, one test binary.
//! See `TESTING.md` at the repository root for the layout and the rules.
//!
//! Each major of a dependency runs the same tests: `utoipa5.rs` / `utoipa6.rs`
//! compile `utoipa/*.rs`, `schemars0.rs` / `schemars1.rs` compile
//! `schemars/*.rs`, against the module under test imported as `bridge`.

mod common;

mod validator;

#[cfg(feature = "poem-openapi")]
mod poem_openapi;
#[cfg(feature = "schemars-0")]
mod schemars0;
#[cfg(feature = "schemars-1")]
mod schemars1;
#[cfg(feature = "utoipa-5")]
mod utoipa5;
#[cfg(feature = "utoipa-6")]
mod utoipa6;
