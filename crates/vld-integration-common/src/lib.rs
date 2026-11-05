//! Shared helpers for `vld` integration crates (internal).
//!
//! Not intended for direct use; the public API lives in the integration
//! crates (`vld-dioxus`, `vld-leptos`, `vld-tauri`, `vld-clap`, DB crates, …)
//! which re-export what they need from here.

use vld::error::{PathSegment, VldError};

pub mod form;

/// Format a validation path as a clean dotted path: `user.email`, `items[0].name`.
///
/// No leading dot; an empty path gives an empty string.
///
/// ```
/// use vld::error::PathSegment;
/// let p = [PathSegment::Field("items".into()), PathSegment::Index(0), PathSegment::Field("name".into())];
/// assert_eq!(vld_integration_common::format_path(&p), "items[0].name");
/// ```
pub fn format_path(path: &[PathSegment]) -> String {
    let mut out = String::new();
    for seg in path {
        match seg {
            PathSegment::Field(name) => {
                if !out.is_empty() {
                    out.push('.');
                }
                out.push_str(name);
            }
            PathSegment::Index(idx) => {
                out.push('[');
                out.push_str(&idx.to_string());
                out.push(']');
            }
        }
    }
    out
}

/// Flatten a [`VldError`] into `(path, message)` pairs using [`format_path`].
pub fn flatten_issues(err: &VldError) -> Vec<(String, String)> {
    err.issues
        .iter()
        .map(|i| (format_path(&i.path), i.message.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_path_has_no_leading_dot() {
        let p = vec![
            PathSegment::Field("user".into()),
            PathSegment::Field("email".into()),
        ];
        assert_eq!(format_path(&p), "user.email");
        assert_eq!(format_path(&[PathSegment::Index(2)]), "[2]");
        assert_eq!(format_path(&[]), "");
    }
}
