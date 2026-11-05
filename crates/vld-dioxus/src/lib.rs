//! # vld-dioxus — Dioxus integration for the `vld` validation library
//!
//! Shared validation for Dioxus server functions and WASM clients.
//! Define validation rules once, use them on both the server and the browser.
//!
//! **Zero dependency on `dioxus`** — works with any Dioxus version (0.5, 0.6, 0.7+).
//! Compatible with WASM and native targets.
//!
//! ## Key Features
//!
//! | Feature | Description |
//! |---|---|
//! | [`validate_args!`] | Inline validation of server function arguments |
//! | [`validate`] / [`validate_value`] | Validate a serializable value against a schema |
//! | [`check_field`] | Single-field validation for reactive UI |
//! | [`check_all_fields`] | Multi-field validation returning per-field errors |
//! | [`VldServerError`] | Serializable error type for server→client error transport |
//!
//! ## Quick Example
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

pub use vld_integration_common::form::{
    check_all_fields, check_field, check_field_all, validate, validate_value, FieldError,
    VldServerError,
};
pub use vld_integration_common::validate_args;

/// Prelude — import everything you need.
pub mod prelude {
    pub use crate::{
        check_all_fields, check_field, check_field_all, validate, validate_args, validate_value,
        FieldError, VldServerError,
    };
    pub use vld::prelude::*;
}
