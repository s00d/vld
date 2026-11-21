//! `vld-app` behaviour tests, one test binary: one module per integration,
//! compiled with its feature(s). See the workspace `TESTING.md`.

#[cfg(feature = "clap")]
mod clap;
#[cfg(any(feature = "config-rs", feature = "figment"))]
mod config;
#[cfg(feature = "env")]
mod env;
mod tauri;
