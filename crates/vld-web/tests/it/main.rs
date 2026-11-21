//! `vld-web` behaviour tests, one test binary: the shared helpers
//! (`common`), the shared HTTP conformance suite (`conformance`) and one
//! module per framework, compiled with its feature. ntex tests are written
//! once (`ntex/`) and compiled per major by `ntex3.rs` / `ntex4.rs`. See the
//! workspace `TESTING.md`.

#[macro_use]
mod conformance;

mod common;

#[cfg(feature = "actix")]
mod actix;
#[cfg(feature = "async-graphql")]
mod async_graphql;
#[cfg(feature = "axum")]
mod axum;
#[cfg(feature = "lambda")]
mod lambda;
#[cfg(feature = "ntex-3")]
mod ntex3;
#[cfg(feature = "ntex-4")]
mod ntex4;
#[cfg(feature = "poem")]
mod poem;
#[cfg(feature = "rama")]
mod rama;
#[cfg(feature = "rig")]
mod rig;
#[cfg(feature = "rmcp")]
mod rmcp;
#[cfg(feature = "rocket")]
mod rocket;
#[cfg(feature = "salvo")]
mod salvo;
#[cfg(feature = "tonic")]
mod tonic;
#[cfg(feature = "tower")]
mod tower;
#[cfg(feature = "tungstenite")]
mod tungstenite;
#[cfg(feature = "warp")]
mod warp;
