//! # `vld_web::async_graphql` — async-graphql 7 integration for `vld`
//!
//! Validate GraphQL input objects (and any other input value) with `vld`
//! schemas. The input is converted to JSON
//! ([`InputType::to_value`] → `ConstValue::into_json`) and parsed by the
//! schema `S`; issues end up in the error's `extensions.issues` with the
//! same `{"path", "message", "code"}` shape as the
//! [HTTP 422 body](crate::common#error-bodies).
//!
//! | Item | Use case |
//! |---|---|
//! | [`VldValidator<S>`] | `#[graphql(validator(custom = "VldValidator::<S>::new()"))]` on an argument or input field |
//! | [`validate`] | Validate inside a resolver: `validate::<S, _>(&input)?` |
//! | [`vld_error`] | Turn a [`VldError`] into an [`async_graphql::Error`] with `extensions.issues` |
//!
//! # Example
//!
//! ```
//! use async_graphql::{EmptyMutation, EmptySubscription, InputObject, Object, Schema};
//! use vld_web::async_graphql::VldValidator;
//!
//! #[derive(InputObject)]
//! struct NewUser {
//!     name: String,
//!     email: String,
//! }
//!
//! // The rules, as a vld schema mirroring the input's JSON.
//! vld::schema! {
//!     pub struct NewUserRules {
//!         pub name: String => vld::string().min(2),
//!         pub email: String => vld::string().email(),
//!     }
//! }
//!
//! struct Query;
//!
//! #[Object]
//! impl Query {
//!     async fn register(
//!         &self,
//!         #[graphql(validator(custom = "VldValidator::<NewUserRules>::new()"))] user: NewUser,
//!     ) -> String {
//!         user.name
//!     }
//! }
//!
//! # tokio_test(async {
//! let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
//! let res = schema
//!     .execute(r#"{ register(user: {name: "A", email: "bad"}) }"#)
//!     .await;
//! let issues = res.errors[0].extensions.as_ref().unwrap().get("issues").unwrap();
//! assert_eq!(issues.clone().into_json().unwrap()[0]["path"], ".name");
//! # });
//! # fn tokio_test(f: impl std::future::Future<Output = ()>) {
//! #     tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(f)
//! # }
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/web/async-graphql>
use std::marker::PhantomData;

use async_graphql::{CustomValidator, ErrorExtensionValues, InputType, InputValueError, Value};
use vld::error::{IssueCode, VldError};
use vld::schema::VldParse;

/// Message of every validation error this module builds.
const MESSAGE: &str = "Validation failed";

/// A `CustomValidator` checking an input value against the vld schema `S`.
///
/// `S` is any [`VldParse`] type (a `vld::schema!` struct, a
/// `#[derive(Validate)]` type) whose rules describe the input's JSON. On
/// failure the GraphQL error has the message
/// `Failed to parse "<Input>": Validation failed` (async-graphql adds the
/// prefix) and `extensions.issues`.
///
/// ```ignore
/// #[graphql(validator(custom = "VldValidator::<NewUserRules>::new()"))] user: NewUser
/// ```
pub struct VldValidator<S>(PhantomData<fn() -> S>);

impl<S> VldValidator<S> {
    /// A validator for the schema `S`.
    #[must_use]
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<S> Default for VldValidator<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: VldParse, T: InputType> CustomValidator<T> for VldValidator<S> {
    fn check(&self, value: &T) -> Result<(), InputValueError<T>> {
        check::<S, T>(value)
            .map_err(|e| InputValueError::custom(MESSAGE).with_extension("issues", issues(&e)))
    }
}

/// Validate an input value against the vld schema `S` inside a resolver.
///
/// ```ignore
/// async fn register(&self, user: NewUser) -> async_graphql::Result<String> {
///     vld_web::async_graphql::validate::<NewUserRules, _>(&user)?;
///     Ok(user.name)
/// }
/// ```
pub fn validate<S: VldParse, T: InputType>(value: &T) -> async_graphql::Result<()> {
    check::<S, T>(value).map_err(|e| vld_error(&e))
}

/// An [`async_graphql::Error`] with message `"Validation failed"` and
/// `extensions.issues`: `[{"path": ".name", "message": "...", "code":
/// "too_small"}]`, the issues of the HTTP 422 body.
pub fn vld_error(error: &VldError) -> async_graphql::Error {
    let mut extensions = ErrorExtensionValues::default();
    extensions.set("issues", issues(error));
    let mut err = async_graphql::Error::new(MESSAGE);
    err.extensions = Some(extensions);
    err
}

fn check<S: VldParse, T: InputType>(value: &T) -> Result<(), VldError> {
    let json = value
        .to_value()
        .into_json()
        .map_err(|e| VldError::single(IssueCode::ParseError, format!("Input is not JSON: {e}")))?;
    S::vld_parse_value(&json).map(drop)
}

fn issues(error: &VldError) -> Value {
    let issues = serde_json::to_value(crate::common::format_issues(error))
        .expect("ValidationIssue serialization cannot fail");
    Value::from_json(issues).expect("issues are plain JSON")
}

/// Prelude — import everything you need.
pub mod prelude {
    pub use super::{VldValidator, validate, vld_error};
    pub use vld::prelude::*;
}
