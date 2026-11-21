//! # `vld_web::ntex4` — ntex 4 integration for `vld`
//!
//! Provides extractors that validate request data using `vld` schemas:
//!
//! | Extractor | Replaces | Source |
//! |---|---|---|
//! | [`VldJson<T>`] | `ntex::web::types::Json<T>` | JSON request body |
//! | [`VldQuery<T>`] | `ntex::web::types::Query<T>` | URL query parameters |
//! | [`VldPath<T>`] | `ntex::web::types::Path<T>` | URL path parameters |
//! | [`VldForm<T>`] | `ntex::web::types::Form<T>` | URL-encoded form body |
//! | [`VldHeaders<T>`] | manual header extraction | HTTP headers |
//! | [`VldCookie<T>`] | manual cookie parsing | Cookie values |
//!
//! All extractors return **422 Unprocessable Entity** on validation failure
//! or malformed input, and **413 Payload Too Large** when the body exceeds
//! the limit. [`VldJson`] requires a JSON `Content-Type` (`application/json`
//! or `application/*+json`) and [`VldForm`]
//! `application/x-www-form-urlencoded`; anything else is **415 Unsupported
//! Media Type**. See the [error body format](crate::common#error-bodies).
//!
//! Guide: <https://s00d.github.io/vld/integrations/web/ntex> (shared rules
//! for all adapters: <https://s00d.github.io/vld/integrations/web/overview>).
//!
//! # Body limits
//!
//! [`VldJson`] and [`VldForm`] read the body themselves up to
//! [`DEFAULT_BODY_LIMIT`](crate::common::DEFAULT_BODY_LIMIT) (2 MiB); ntex's
//! `JsonConfig` / `PayloadConfig` do not apply. Register [`VldBodyLimit`] as
//! app state to change it: `App::new().with_config(WebAppConfig::new().set_state(VldBodyLimit(n)))`. Compressed request bodies are decoded only
//! with the `ntex-compress` feature (the limit then applies to the decoded
//! size).
//!
//! # ntex versions
//!
//! This module is ntex 4 (feature `ntex-4`); `vld_web::ntex3` is the same
//! adapter for ntex 3 (feature `ntex-3`). Both can be enabled together.
//! The ntex crate is re-exported as [`ntex`].
//!
//! # Example
//!
//! ```no_run
//! use vld_web::ntex4::ntex::web::HttpResponse;
//! use vld_web::ntex4::{VldJson, VldPath};
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct PathParams {
//!         pub id: i64 => vld::number().int().min(1),
//!     }
//! }
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct Body {
//!         pub name: String => vld::string().min(2),
//!     }
//! }
//!
//! async fn handler(
//!     path: VldPath<PathParams>,
//!     body: VldJson<Body>,
//! ) -> HttpResponse {
//!     HttpResponse::Ok().body(format!("id={} name={}", path.id, body.name))
//! }
//!
//! // Inside `HttpServer::new(...)`:
//! // App::new().route("/users/{id}", web::post().to(handler))
//! ```

/// The ntex 4 crate this module is built on.
pub use ::ntex4 as ntex;

// One implementation, compiled once per ntex major (ARCHITECTURE.md).
#[allow(clippy::duplicate_mod)]
#[path = "ntex/shared.rs"]
mod shared;
pub use shared::*;

/// ntex 4 glue: error rendering and the `FromRequest` impls (forwarding to
/// each extractor's inherent `extract`).
mod compat {
    use super::ntex;
    use super::shared::*;
    use ntex::http::Payload;
    use ntex::web::FromRequest;
    use ntex::web::error::WebResponseError;
    use ntex::web::{HttpRequest, HttpResponse};
    use vld::schema::VldParse;

    impl<St, Err> WebResponseError<St, Err> for VldNtexError {
        fn error_response(&self, _: &St) -> HttpResponse {
            self.render()
        }
    }

    macro_rules! from_request {
        ($($t:ident),+) => {$(
            impl<T: VldParse, St> FromRequest<St> for $t<T> {
                type Error = VldNtexError;

                async fn from_request(
                    _: &St,
                    req: &HttpRequest,
                    payload: &mut Payload,
                ) -> Result<Self, Self::Error> {
                    Self::extract(req, payload).await
                }
            }
        )+};
    }

    from_request!(VldJson, VldQuery, VldPath, VldForm, VldHeaders, VldCookie);
}
