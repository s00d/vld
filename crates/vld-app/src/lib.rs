//! # vld-app — application-edge integrations for the `vld` validation library
//!
//! Validate what an application reads at its edges with `vld` schemas, one
//! module per integration. Every library-backed module is behind its
//! feature; none is on by default:
//!
//! ```toml
//! vld-app = { version = "0.8", features = ["config-rs", "clap"] }
//! ```
//!
//! | Module | Feature | Library |
//! |---|---|---|
//! | `config` | `config-rs` and / or `figment` | config-rs 0.14 – 0.15, figment 0.10 |
//! | `clap` | `clap` | clap 4 argument validation (enables `vld/derive`) |
//! | [`tauri`] | — | Tauri IPC payloads (no `tauri` dependency) |
//!
//! ```ignore
//! use vld_app::config::from_config;
//! ```
#![warn(missing_docs)]

#[cfg(feature = "clap")]
pub mod clap;
#[cfg(any(feature = "config-rs", feature = "figment"))]
pub mod config;
pub mod tauri;
