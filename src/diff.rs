//! Schema diffing — compare two JSON Schemas to find breaking changes.
//!
//! Useful for API versioning: detect when a schema change will break
//! existing consumers.
//!
//! Requires the `openapi` feature (schemas are represented as
//! `serde_json::Value` in JSON Schema format).
//!
//! # Example
//!
//! ```
//! use serde_json::json;
//! use vld::diff::{diff_schemas, ChangeKind};
//!
//! let old = json!({
//!     "type": "object",
//!     "required": ["name", "email"],
//!     "properties": {
//!         "name": { "type": "string" },
//!         "email": { "type": "string", "format": "email" }
//!     }
//! });
//!
//! let new = json!({
//!     "type": "object",
//!     "required": ["name", "email", "age"],
//!     "properties": {
//!         "name": { "type": "string" },
//!         "email": { "type": "string" },
//!         "age": { "type": "integer" }
//!     }
//! });
//!
//! let changes = diff_schemas(&old, &new);
//! assert!(changes.has_breaking());
//! ```

use serde_json::Value;
use std::collections::BTreeSet;
use std::fmt;

/// Severity of a schema change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChangeKind {
    /// Non-breaking: new optional fields, relaxed constraints, etc.
    NonBreaking,
    /// Breaking: removed fields, added required fields, tightened constraints, etc.
    Breaking,
}

impl fmt::Display for ChangeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChangeKind::NonBreaking => write!(f, "non-breaking"),
            ChangeKind::Breaking => write!(f, "BREAKING"),
        }
    }
}

/// A single schema change.
#[derive(Debug, Clone)]
pub struct SchemaChange {
    /// Dot-separated path to the changed element (e.g. `"properties.email.format"`).
    pub path: String,
    /// Severity.
    pub kind: ChangeKind,
    /// Human-readable description.
    pub description: String,
}

impl fmt::Display for SchemaChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.kind, self.path, self.description)
    }
}

/// Result of diffing two schemas.
#[derive(Debug, Clone)]
pub struct SchemaDiff {
    pub changes: Vec<SchemaChange>,
}

impl SchemaDiff {
    /// Whether any breaking changes were detected.
    pub fn has_breaking(&self) -> bool {
        self.changes.iter().any(|c| c.kind == ChangeKind::Breaking)
    }

    /// Only the breaking changes.
    pub fn breaking_changes(&self) -> Vec<&SchemaChange> {
        self.changes
            .iter()
            .filter(|c| c.kind == ChangeKind::Breaking)
            .collect()
    }

    /// Only the non-breaking changes.
    pub fn non_breaking_changes(&self) -> Vec<&SchemaChange> {
        self.changes
            .iter()
            .filter(|c| c.kind == ChangeKind::NonBreaking)
            .collect()
    }
}

impl fmt::Display for SchemaDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.changes.is_empty() {
            writeln!(f, "No changes detected.")?;
        } else {
            for change in &self.changes {
                writeln!(f, "{}", change)?;
            }
        }
        Ok(())
    }
}

/// Compare two JSON Schemas and return a list of changes.
///
/// The comparison is heuristic and focuses on the most common JSON Schema
/// properties: `type`, `required`, `properties`, `format`, `minimum`,
/// `maximum`, `minLength`, `maxLength`, `minItems`, `maxItems`, `enum`, `pattern`.
pub fn diff_schemas(old: &Value, new: &Value) -> SchemaDiff {
    let mut changes = Vec::new();
    diff_value(old, new, "", &mut changes);
    SchemaDiff { changes }
}

fn diff_value(old: &Value, new: &Value, path: &str, changes: &mut Vec<SchemaChange>) {
    // Type change
    if old.get("type") != new.get("type") {
        changes.push(SchemaChange {
            path: join_path(path, "type"),
            kind: ChangeKind::Breaking,
            description: format!(
                "Type changed from {} to {}",
                fmt_val(old.get("type")),
                fmt_val(new.get("type"))
            ),
        });
        return; // If type changed, comparing sub-properties is not meaningful
    }

    // Required fields
    diff_required(old, new, path, changes);

    // Properties
    diff_properties(old, new, path, changes);

    // Numeric constraints
    diff_numeric_constraint(old, new, path, "minimum", true, changes);
    diff_numeric_constraint(old, new, path, "maximum", false, changes);
    diff_numeric_constraint(old, new, path, "exclusiveMinimum", true, changes);
    diff_numeric_constraint(old, new, path, "exclusiveMaximum", false, changes);
    diff_numeric_constraint(old, new, path, "minLength", true, changes);
    diff_numeric_constraint(old, new, path, "maxLength", false, changes);
    diff_numeric_constraint(old, new, path, "minItems", true, changes);
    diff_numeric_constraint(old, new, path, "maxItems", false, changes);

    // Format
    if old.get("format") != new.get("format") {
        let now_none = new.get("format").is_none();
        let kind = if now_none {
            ChangeKind::NonBreaking // relaxed
        } else {
            ChangeKind::Breaking // added or changed constraint
        };
        changes.push(SchemaChange {
            path: join_path(path, "format"),
            kind,
            description: format!(
                "Format changed from {} to {}",
                fmt_val(old.get("format")),
                fmt_val(new.get("format"))
            ),
        });
    }

    // Pattern
    if old.get("pattern") != new.get("pattern") {
        let kind = if new.get("pattern").is_none() {
            ChangeKind::NonBreaking
        } else {
            ChangeKind::Breaking
        };
        changes.push(SchemaChange {
            path: join_path(path, "pattern"),
            kind,
            description: format!(
                "Pattern changed from {} to {}",
                fmt_val(old.get("pattern")),
                fmt_val(new.get("pattern"))
            ),
        });
    }

    // Enum
    diff_enum(old, new, path, changes);

    // Items (array element schema)
    if let (Some(old_items), Some(new_items)) = (old.get("items"), new.get("items")) {
        diff_value(old_items, new_items, &join_path(path, "items"), changes);
    }

    // AdditionalProperties
    if old.get("additionalProperties") != new.get("additionalProperties") {
        let old_ap = old.get("additionalProperties");
        let new_ap = new.get("additionalProperties");
        // false → true is non-breaking; true → false is breaking
        let kind = match (old_ap, new_ap) {
            (Some(Value::Bool(true)), Some(Value::Bool(false))) => ChangeKind::Breaking,
            (Some(Value::Bool(false)), Some(Value::Bool(true))) => ChangeKind::NonBreaking,
            (None, Some(Value::Bool(false))) => ChangeKind::Breaking,
            _ => ChangeKind::NonBreaking,
        };
        changes.push(SchemaChange {
            path: join_path(path, "additionalProperties"),
            kind,
            description: format!(
                "additionalProperties changed from {} to {}",
                fmt_val(old_ap),
                fmt_val(new_ap)
            ),
        });
    }
}

fn diff_required(old: &Value, new: &Value, path: &str, changes: &mut Vec<SchemaChange>) {
    let old_req = extract_string_set(old.get("required"));
    let new_req = extract_string_set(new.get("required"));

    for added in new_req.difference(&old_req) {
        // New required field — need to check if it's also a new property
        let is_new_prop = old
            .get("properties")
            .and_then(|p| p.as_object())
            .map(|p| !p.contains_key(added.as_str()))
            .unwrap_or(true);

        let kind = if is_new_prop {
            // New required property without a default — breaking
            ChangeKind::Breaking
        } else {
            // Existing optional field made required — breaking
            ChangeKind::Breaking
        };

        changes.push(SchemaChange {
            path: join_path(path, &format!("required[{}]", added)),
            kind,
            description: format!("Field \"{}\" is now required", added),
        });
    }

    for removed in old_req.difference(&new_req) {
        // Field no longer required — non-breaking
        let prop_removed = new
            .get("properties")
            .and_then(|p| p.as_object())
            .map(|p| !p.contains_key(removed.as_str()))
            .unwrap_or(false);

        if prop_removed {
            // Field removed entirely — that's handled in diff_properties
        } else {
            changes.push(SchemaChange {
                path: join_path(path, &format!("required[{}]", removed)),
                kind: ChangeKind::NonBreaking,
                description: format!("Field \"{}\" is no longer required", removed),
            });
        }
    }
}

fn diff_properties(old: &Value, new: &Value, path: &str, changes: &mut Vec<SchemaChange>) {
    let old_props = old.get("properties").and_then(Value::as_object);
    let new_props = new.get("properties").and_then(Value::as_object);

    let (old_props, new_props) = match (old_props, new_props) {
        (Some(a), Some(b)) => (a, b),
        (None, None) => return,
        (Some(_), None) => {
            changes.push(SchemaChange {
                path: join_path(path, "properties"),
                kind: ChangeKind::Breaking,
                description: "Properties removed entirely".into(),
            });
            return;
        }
        (None, Some(_)) => {
            changes.push(SchemaChange {
                path: join_path(path, "properties"),
                kind: ChangeKind::NonBreaking,
                description: "Properties added".into(),
            });
            return;
        }
    };

    let new_req = extract_string_set(new.get("required"));

    // Removed properties
    for key in old_props.keys() {
        if !new_props.contains_key(key) {
            changes.push(SchemaChange {
                path: join_path(path, &format!("properties.{}", key)),
                kind: ChangeKind::Breaking,
                description: format!("Property \"{}\" removed", key),
            });
        }
    }

    // Added properties
    for key in new_props.keys() {
        if !old_props.contains_key(key) {
            let kind = if new_req.contains(key) {
                ChangeKind::Breaking
            } else {
                ChangeKind::NonBreaking
            };
            changes.push(SchemaChange {
                path: join_path(path, &format!("properties.{}", key)),
                kind,
                description: format!("Property \"{}\" added", key),
            });
        }
    }

    // Changed properties
    for (key, old_val) in old_props {
        if let Some(new_val) = new_props.get(key) {
            diff_value(
                old_val,
                new_val,
                &join_path(path, &format!("properties.{}", key)),
                changes,
            );
        }
    }
}

fn diff_numeric_constraint(
    old: &Value,
    new: &Value,
    path: &str,
    key: &str,
    is_lower_bound: bool,
    changes: &mut Vec<SchemaChange>,
) {
    let old_v = old.get(key).and_then(Value::as_f64);
    let new_v = new.get(key).and_then(Value::as_f64);

    match (old_v, new_v) {
        (Some(a), Some(b)) if (a - b).abs() > f64::EPSILON => {
            let kind = if is_lower_bound {
                if b > a {
                    ChangeKind::Breaking
                } else {
                    ChangeKind::NonBreaking
                }
            } else if b < a {
                ChangeKind::Breaking
            } else {
                ChangeKind::NonBreaking
            };
            changes.push(SchemaChange {
                path: join_path(path, key),
                kind,
                description: format!("{} changed from {} to {}", key, a, b),
            });
        }
        (None, Some(v)) => {
            changes.push(SchemaChange {
                path: join_path(path, key),
                kind: ChangeKind::Breaking,
                description: format!("{} constraint added: {}", key, v),
            });
        }
        (Some(v), None) => {
            changes.push(SchemaChange {
                path: join_path(path, key),
                kind: ChangeKind::NonBreaking,
                description: format!("{} constraint removed (was {})", key, v),
            });
        }
        _ => {}
    }
}

fn diff_enum(old: &Value, new: &Value, path: &str, changes: &mut Vec<SchemaChange>) {
    let old_enum = old.get("enum").and_then(Value::as_array);
    let new_enum = new.get("enum").and_then(Value::as_array);

    match (old_enum, new_enum) {
        (Some(old_e), Some(new_e)) => {
            let old_set: BTreeSet<String> = old_e.iter().map(|v| v.to_string()).collect();
            let new_set: BTreeSet<String> = new_e.iter().map(|v| v.to_string()).collect();

            for removed in old_set.difference(&new_set) {
                changes.push(SchemaChange {
                    path: join_path(path, "enum"),
                    kind: ChangeKind::Breaking,
                    description: format!("Enum value {} removed", removed),
                });
            }

            for added in new_set.difference(&old_set) {
                changes.push(SchemaChange {
                    path: join_path(path, "enum"),
                    kind: ChangeKind::NonBreaking,
                    description: format!("Enum value {} added", added),
                });
            }
        }
        (None, Some(_)) => {
            changes.push(SchemaChange {
                path: join_path(path, "enum"),
                kind: ChangeKind::Breaking,
                description: "Enum constraint added".into(),
            });
        }
        (Some(_), None) => {
            changes.push(SchemaChange {
                path: join_path(path, "enum"),
                kind: ChangeKind::NonBreaking,
                description: "Enum constraint removed".into(),
            });
        }
        _ => {}
    }
}

fn extract_string_set(value: Option<&Value>) -> BTreeSet<String> {
    value
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

fn join_path(base: &str, segment: &str) -> String {
    if base.is_empty() {
        segment.to_string()
    } else {
        format!("{}.{}", base, segment)
    }
}

fn fmt_val(v: Option<&Value>) -> String {
    match v {
        Some(val) => val.to_string(),
        None => "(none)".to_string(),
    }
}
