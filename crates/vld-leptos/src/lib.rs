//! # vld-leptos — Leptos helpers for `vld`
//!
//! Shared validation for Leptos server functions and WASM clients: define the
//! rules once, check them on the server and in the browser.
//!
//! No dependency on `leptos`, so any Leptos version works; builds for native
//! and WASM targets.
//!
//! | Item | Use |
//! |---|---|
//! | [`validate_args!`] | Validate server-function arguments inline |
//! | [`validate`] / [`validate_value`] | Validate a value against a schema type |
//! | [`check_field`] / [`check_field_all`] | Single-field validation for reactive UI |
//! | [`check_all_fields`] | Whole-form validation returning per-field errors |
//! | [`VldServerError`] | Serializable error for server → client transport |
//!
//! Field paths have no leading dot (`name`, `user.email`, `items[0]`).
//!
//! ```
//! vld::schema! {
//!     struct CreateUserSchema {
//!         name: String => vld::string().min(2).max(50),
//!         email: String => vld::string().email(),
//!     }
//! }
//!
//! #[derive(serde::Serialize)]
//! struct CreateUserArgs { name: String, email: String }
//!
//! let args = CreateUserArgs { name: "X".into(), email: "bad".into() };
//! let errors = vld_leptos::check_all_fields::<CreateUserSchema, _>(&args);
//! assert_eq!(errors.len(), 2);
//! assert_eq!(errors[0].field, "name");
//! ```
//!
//! In an app:
//!
//! ```ignore
//! // Shared validation rules (used on server AND client)
//! fn name_schema() -> vld::primitives::ZString { vld::string().min(2).max(50) }
//! fn email_schema() -> vld::primitives::ZString { vld::string().email() }
//!
//! // Server function
//! #[server]
//! async fn create_user(name: String, email: String) -> Result<(), ServerFnError> {
//!     vld_leptos::validate_args! {
//!         name  => name_schema(),
//!         email => email_schema(),
//!     }.map_err(|e| ServerFnError::new(e.to_string()))?;
//!     // ... create user in database
//!     Ok(())
//! }
//!
//! // Client component (reactive validation)
//! #[component]
//! fn CreateUserForm() -> impl IntoView {
//!     let (name, set_name) = signal(String::new());
//!     let name_err = Memo::new(move |_| {
//!         vld_leptos::check_field(&name.get(), &name_schema())
//!     });
//!     // ... render form with error display
//! }
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/ui/leptos>
#![warn(missing_docs)]

pub use vld_integration_common::form::{
    FieldError, VldServerError, check_all_fields, check_field, check_field_all, validate,
    validate_value,
};
pub use vld_integration_common::validate_args;

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{
        FieldError, VldServerError, check_all_fields, check_field, check_field_all, validate,
        validate_args, validate_value,
    };
    pub use vld::prelude::*;
}
