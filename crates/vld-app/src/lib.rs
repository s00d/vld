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
//! | `env` | `env` and / or `dotenv` | environment variables (std); `.env` files (dotenvy 0.15) |
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
#[cfg(feature = "env")]
pub mod env;
pub mod tauri;
