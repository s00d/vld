//! `vld-dioxus` behaviour tests: one test binary.
//!
//! The crate re-exports the form helpers of `vld-integration-common`; the
//! stories call them the way a Dioxus app does.
//!
//! | File | Kind | What |
//! |---|---|---|
//! | `story_form.rs` | stories | A form validated on the client: one field at a time as the user types (`check_field`, `check_field_all`), the whole form on submit (`check_all_fields`, `validate`, `validate_value`), nested / list field paths, unserializable values. |
//! | `story_server_fn.rs` | stories | A server function validating its arguments (`validate_args!`), the error sent as text and read back on the client (`from_json`, `from_server_message`), the per-field accessors, errors built by hand. |
//!
//! Compared: field names and their order, which fields fail; message texts
//! only for the summary and where a field's messages are listed.

mod story_form;
mod story_server_fn;
