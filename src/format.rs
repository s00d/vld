use std::collections::HashMap;

use crate::error::VldError;

/// Flat error structure, useful for form validation.
///
/// - `form_errors`: top-level errors (no path)
/// - `field_errors`: errors grouped by top-level field name
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "deserialize", derive(serde::Deserialize))]
pub struct FlatError {
    pub form_errors: Vec<String>,
    pub field_errors: HashMap<String, Vec<String>>,
}

/// Flatten a `VldError` into a simple field-based structure.
///
/// # Example
/// ```
/// use vld::format::flatten_error;
/// use vld::error::VldError;
///
/// let err = VldError::new(); // empty for demo
/// let flat = flatten_error(&err);
/// assert!(flat.form_errors.is_empty());
/// ```
pub fn flatten_error(error: &VldError) -> FlatError {
    let mut form_errors = Vec::new();
    let mut field_errors: HashMap<String, Vec<String>> = HashMap::new();

    for issue in &error.issues {
        if issue.path.is_empty() {
            form_errors.push(issue.message.clone());
        } else {
            let key = issue.path[0].to_string();
            // Strip leading dot from field names
            let key = key.strip_prefix('.').unwrap_or(&key).to_string();
            field_errors
                .entry(key)
                .or_default()
                .push(issue.message.clone());
        }
    }

    FlatError {
        form_errors,
        field_errors,
    }
}

/// Tree-based error structure, mirrors the schema shape.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "deserialize", derive(serde::Deserialize))]
pub struct ErrorTree {
    pub errors: Vec<String>,
    pub properties: HashMap<String, ErrorTree>,
    pub items: Vec<Option<ErrorTree>>,
}

/// Convert a `VldError` into a tree structure that mirrors the schema.
pub fn treeify_error(error: &VldError) -> ErrorTree {
    let mut root = ErrorTree::default();

    for issue in &error.issues {
        if issue.path.is_empty() {
            root.errors.push(issue.message.clone());
            continue;
        }

        let mut current = &mut root;

        for (i, segment) in issue.path.iter().enumerate() {
            let is_last = i == issue.path.len() - 1;

            match segment {
                crate::error::PathSegment::Field(name) => {
                    if !current.properties.contains_key(name) {
                        current
                            .properties
                            .insert(name.clone(), ErrorTree::default());
                    }
                    if is_last {
                        current
                            .properties
                            .get_mut(name)
                            .unwrap()
                            .errors
                            .push(issue.message.clone());
                    } else {
                        current = current.properties.get_mut(name).unwrap();
                    }
                }
                crate::error::PathSegment::Index(idx) => {
                    while current.items.len() <= *idx {
                        current.items.push(None);
                    }
                    if current.items[*idx].is_none() {
                        current.items[*idx] = Some(ErrorTree::default());
                    }
                    if is_last {
                        current.items[*idx]
                            .as_mut()
                            .unwrap()
                            .errors
                            .push(issue.message.clone());
                    } else {
                        current = current.items[*idx].as_mut().unwrap();
                    }
                }
            }
        }
    }

    root
}

/// Format a `VldError` into a human-readable string.
///
/// # Example output
/// ```text
/// ✖ String must be at least 2 characters
///   → at .name
/// ✖ Invalid email address
///   → at .email
/// ```
pub fn prettify_error(error: &VldError) -> String {
    let mut lines = Vec::new();

    for issue in &error.issues {
        lines.push(format!("✖ {}", issue.message));
        if !issue.path.is_empty() || issue.received.is_some() {
            let mut parts = Vec::new();
            if !issue.path.is_empty() {
                let path_str: String = issue.path.iter().map(|p| p.to_string()).collect();
                parts.push(format!("at {}", path_str));
            }
            if let Some(val) = &issue.received {
                parts.push(format!(
                    "received {}",
                    crate::error::format_value_short(val)
                ));
            }
            lines.push(format!("  → {}", parts.join(", ")));
        }
    }

    lines.join("\n")
}
