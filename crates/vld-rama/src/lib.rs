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
//! the limit. [`VldJson`] requires a JSON `Content-Type` (`application/json`
//! or `application/*+json`) and [`VldForm`]
//! `application/x-www-form-urlencoded`; anything else is **415 Unsupported
//! Media Type**. [`ValidateJsonLayer`] passes non-JSON requests through.
//! See the [error body format](vld_http_common#error-bodies).
//!
//! # Body limit
//!
//! [`DEFAULT_BODY_LIMIT`](vld_http_common::DEFAULT_BODY_LIMIT) (2 MiB).
//! The extractors read the [`VldBodyLimit`] request extension, e.g. added
//! with `rama::layer::AddInputExtensionLayer::new(VldBodyLimit(n))`; the
//! layer takes [`ValidateJsonLayer::with_limit`].
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

/// Body size limit (bytes) for [`VldJson`] and [`VldForm`], read from the
/// request extensions; without it they use
/// [`DEFAULT_BODY_LIMIT`](vld_http_common::DEFAULT_BODY_LIMIT) (2 MiB).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VldBodyLimit(pub usize);

impl rama::extensions::Extension for VldBodyLimit {}

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{
        validated, ValidateJsonLayer, ValidateJsonService, Validated, VldBodyLimit, VldCookie,
        VldForm, VldHeaders, VldJson, VldPath, VldQuery, VldRejection,
    };
    pub use vld::prelude::*;
}
