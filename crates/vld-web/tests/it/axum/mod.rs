//! `vld_web::axum` behaviour tests: requests through a real axum `Router`.

mod conformance;
mod story_api;
mod story_loco;
#[cfg(feature = "axum-ws")]
mod story_ws;
