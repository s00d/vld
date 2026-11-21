//! # vld-web — HTTP integrations for the `vld` validation library
//!
//! Extractors, filters and error responses that validate request data with
//! `vld` schemas, one module per web framework. Every module is behind its
//! feature; none is on by default:
//!
//! ```toml
//! vld-web = { version = "0.8", features = ["axum"] }
//! ```
//!
//! | Module | Feature | Framework |
//! |---|---|---|
//! | [`common`] | — | framework-agnostic parsing helpers and the shared error type, for custom adapters |
//! | `axum` | `axum` | axum 0.8 extractors |
//! | `actix` | `actix` | actix-web 4 extractors |
//! | `ntex3` / `ntex4` | `ntex-3` / `ntex-4` (+ `ntex-compress`) | ntex 3 / ntex 4 extractors |
//! | `poem` | `poem` | poem 3 extractors |
//! | `rocket` | `rocket` | Rocket 0.5 data guards, request guards and catchers |
//! | `salvo` | `salvo` | salvo 1 extractors |
//! | `warp` | `warp` | warp 0.3 – 0.4 filters |
//! | `rama` | `rama` | rama 0.4 extractors and a validation layer |
//! | `tower` | `tower` | a Tower layer validating JSON bodies (any Tower-based framework) |
//! | `tonic` | `tonic` | tonic 0.12 – 0.14 message and metadata validation |
//! | `async_graphql` | `async-graphql` | async-graphql 7 input validator and errors with `extensions.issues` |
//! | `tungstenite` | `tungstenite` | tungstenite 0.26 – 0.30 WebSocket messages (also tokio-/async-tungstenite) |
//! | `axum::ws` | `axum-ws` | axum WebSocket messages |
//! | `lambda` | `lambda` | AWS Lambda: `lambda_http` 1 requests and raw `lambda_runtime` events |
//! | `rmcp` | `rmcp` | MCP tools (rmcp 3): vld-validated arguments, `isError` results with model feedback |
//! | `rig` | `rig` | rig 0.47 tools: vld-validated arguments, schema per LLM provider |
//!
//! Every HTTP module answers with the same error bodies (422 / 413 / 415,
//! see [`common#error-bodies`](common#error-bodies)) and parses query
//! strings, forms, paths, headers and cookies by the same rules.
//!
//! ```ignore
//! use vld_web::axum::{VldJson, VldQuery};
//! ```
//!
//! Some features need a newer compiler than the crate's `rust-version`
//! (see MSRV.md); cargo reports the framework's `rust-version` then.
#![warn(missing_docs)]

/// Implement `Deref`/`DerefMut` to the inner value for tuple extractors
/// `Name<T>(pub T)`.
#[allow(unused_macros)]
macro_rules! impl_deref {
    ($($name:ident),+ $(,)?) => {$(
        impl<T> ::std::ops::Deref for $name<T> {
            type Target = T;
            fn deref(&self) -> &T {
                &self.0
            }
        }
        impl<T> ::std::ops::DerefMut for $name<T> {
            fn deref_mut(&mut self) -> &mut T {
                &mut self.0
            }
        }
    )+};
}
#[allow(unused_imports)]
pub(crate) use impl_deref;

pub mod common;

#[cfg(any(feature = "tungstenite", feature = "axum-ws"))]
mod ws;

#[cfg(feature = "actix")]
pub mod actix;
#[cfg(feature = "async-graphql")]
pub mod async_graphql;
#[cfg(feature = "axum")]
pub mod axum;
#[cfg(feature = "lambda")]
pub mod lambda;
#[cfg(feature = "ntex-3")]
pub mod ntex3;
#[cfg(feature = "ntex-4")]
pub mod ntex4;
#[cfg(feature = "poem")]
pub mod poem;
#[cfg(feature = "rama")]
pub mod rama;
#[cfg(feature = "rig")]
pub mod rig;
#[cfg(feature = "rmcp")]
pub mod rmcp;
#[cfg(feature = "rocket")]
pub mod rocket;
#[cfg(feature = "salvo")]
pub mod salvo;
#[cfg(feature = "tonic")]
pub mod tonic;
#[cfg(feature = "tower")]
pub mod tower;
#[cfg(feature = "tungstenite")]
pub mod tungstenite;
#[cfg(feature = "warp")]
pub mod warp;
