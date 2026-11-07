//! # vld-rama — Rama integration for `vld`
//!
//! Extractors and a [`ValidateJsonLayer`] that validate HTTP request data
//! with `vld` schemas (no `serde::Deserialize` needed on your types).
//!
//! | API | Source |
//! |---|---|
//! | [`VldJson<T>`] | JSON request body |
//! | [`VldQuery<T>`] | URL query parameters |
//! | [`VldPath<T>`] | URL path parameters |
//! | [`VldForm<T>`] | URL-encoded form body |
//! | [`VldHeaders<T>`] | HTTP headers |
//! | [`VldCookie<T>`] | Cookie values |
//! | [`ValidateJsonLayer<T>`] | Middleware: validates JSON bodies, stores [`Validated<T>`] |
//!
//! Failures are JSON responses (see the
//! [error body format](vld_http_common#error-bodies)): **422** for invalid or
//! malformed input, **413** for a body over the limit, **415** when
//! [`VldJson`] gets no JSON `Content-Type` (`application/json` or
//! `application/*+json`) or [`VldForm`] no
//! `application/x-www-form-urlencoded`. [`ValidateJsonLayer`] passes
//! non-JSON requests through, so [`validated`] returns `Option<T>`.
//!
//! # Example
//!
//! ```
//! use rama::http::service::web::WebService;
//! use rama::http::{Body, Request, StatusCode};
//! use rama::Service;
//! use vld_rama::VldJson;
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct CreateUser {
//!         pub name: String => vld::string().min(2),
//!         pub email: String => vld::string().email(),
//!     }
//! }
//!
//! async fn create(VldJson(user): VldJson<CreateUser>) -> String {
//!     format!("created {}", user.name)
//! }
//!
//! # #[tokio::main]
//! # async fn main() {
//! let svc = WebService::default().with_post("/users", create);
//!
//! let req = Request::builder()
//!     .method("POST")
//!     .uri("http://example.com/users")
//!     .header("content-type", "application/json")
//!     .body(Body::from(r#"{"name":"A","email":"bad"}"#))
//!     .unwrap();
//! let resp = svc.serve(req).await.unwrap();
//! assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
//! # }
//! ```
//!
//! # Body limit
//!
//! [`DEFAULT_BODY_LIMIT`](vld_http_common::DEFAULT_BODY_LIMIT) (2 MiB).
//! The extractors read the [`VldBodyLimit`] request extension, e.g. added
//! with `rama::layer::AddInputExtensionLayer::new(VldBodyLimit(n))`; the
//! layer takes [`ValidateJsonLayer::with_limit`].
//!
//! Requires Rust **1.96+** (Rama's MSRV).
//!
//! Guide: <https://s00d.github.io/vld/integrations/web/rama>
#![warn(missing_docs)]

mod extract;
mod layer;
mod rejection;

pub use extract::{VldCookie, VldForm, VldHeaders, VldJson, VldPath, VldQuery};
pub use layer::{ValidateJsonLayer, ValidateJsonService, Validated, validated};
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
        ValidateJsonLayer, ValidateJsonService, Validated, VldBodyLimit, VldCookie, VldForm,
        VldHeaders, VldJson, VldPath, VldQuery, VldRejection, validated,
    };
    pub use vld::prelude::*;
}
