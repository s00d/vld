//! # vld-dioxus — Dioxus helpers for `vld`
//!
//! Shared validation for Dioxus server functions and WASM clients: define the
//! rules once, check them on the server and in the browser.
//!
//! No dependency on `dioxus`, so any Dioxus version works; builds for native
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
//! fn name_schema() -> vld::primitives::ZString { vld::string().min(2).max(50) }
//! fn email_schema() -> vld::primitives::ZString { vld::string().email() }
//!
//! let name = "A".to_string();
//! let email = "bad".to_string();
//! let err = vld_dioxus::validate_args! {
//!     name  => name_schema(),
//!     email => email_schema(),
//! }
//! .unwrap_err();
//! assert_eq!(err.error_fields(), ["name", "email"]);
//!
//! // The client side gets it back from `ServerFnError`'s message:
//! let shown = format!("error running server function: {err}");
//! let back = vld_dioxus::VldServerError::from_server_message(&shown).unwrap();
//! assert!(back.has_field_error("email"));
//!
//! assert!(vld_dioxus::check_field(&"Alice".to_string(), &name_schema()).is_none());
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
//!     vld_dioxus::validate_args! {
//!         name  => name_schema(),
//!         email => email_schema(),
//!     }.map_err(|e| ServerFnError::new(e.to_string()))?;
//!     Ok(())
//! }
//!
//! // Client component (reactive validation)
//! #[component]
//! fn CreateUserForm() -> Element {
//!     let mut name = use_signal(String::new);
//!     let name_err = use_memo(move || {
//!         vld_dioxus::check_field(&name(), &name_schema())
//!     });
//!     // ... render form with error display
//! }
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/ui/dioxus>
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
