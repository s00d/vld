//! # vld-rama — Rama integration for the `vld` validation library
//!
//! Provides extractors and a [`ValidateJsonLayer`] that validate HTTP request
//! data using `vld` schemas (no `serde::Deserialize` required):
//!
//! | API | Source |
//! |---|---|
//! | [`VldJson<T>`] | JSON request body |
//! | [`VldQuery<T>`] | URL query parameters |
//! | [`VldPath<T>`] | URL path parameters |
//! | [`VldForm<T>`] | URL-encoded form body |
//! | [`VldHeaders<T>`] | HTTP headers |
//! | [`VldCookie<T>`] | Cookie values |
//! | [`ValidateJsonLayer<T>`] | Middleware for Service stacks |
//!
//! All extractors return **422 Unprocessable Entity** on validation failure
//! or malformed input, and **413 Payload Too Large** when a body exceeds
//! [`DEFAULT_BODY_LIMIT`](vld_http_common::DEFAULT_BODY_LIMIT) (2 MiB;
//! configurable on the layer via [`ValidateJsonLayer::with_limit`]).
//! [`VldJson`] requires a JSON `Content-Type` (`application/json` or
//! `application/*+json`) and answers **415 Unsupported Media Type**
//! otherwise; [`ValidateJsonLayer`] passes non-JSON requests through.
//!
//! Requires Rust **1.96+** (Rama MSRV).

mod extract;
mod layer;
mod rejection;

pub use extract::{VldCookie, VldForm, VldHeaders, VldJson, VldPath, VldQuery};
#[allow(deprecated)]
pub use layer::try_validated;
pub use layer::{validated, ValidateJsonLayer, ValidateJsonService, Validated};
pub use rejection::VldRejection;

/// Prelude — import everything you need.
pub mod prelude {
    #[allow(deprecated)]
    pub use crate::try_validated;
    pub use crate::{
        validated, ValidateJsonLayer, ValidateJsonService, Validated, VldCookie, VldForm,
        VldHeaders, VldJson, VldPath, VldQuery, VldRejection,
    };
    pub use vld::prelude::*;
}
