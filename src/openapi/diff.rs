//! Schema diffing — compare two JSON Schemas to find breaking changes.
//!
//! Useful for API versioning: detect when a schema change will break
//! existing consumers.
//!
//! Requires the `diff` feature. It works on plain `serde_json::Value` JSON
//! Schemas (e.g. from `json_schema()` with the `openapi` feature, or any other
//! source).
//!
//! **Direction**: the classification assumes request / input schemas — a
//! change is breaking when the new schema rejects input the old one accepted
//! (stricter), non-breaking when it only accepts more. For response schemas
//! read it the other way round.
//!
//! Nullable wrappers (`anyOf`/`oneOf` of `[X, {"type": "null"}]`, as emitted
//! by `.optional()` / `.nullable()`) are unwrapped and `X` is compared;
//! gaining `null` is non-breaking, losing it is breaking. `anyOf` / `oneOf` /
//! `allOf` branches are compared position by position, `$ref` targets by name.
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
/// The comparison is heuristic and covers: `type`, `required`, `properties`,
/// `format`, `minimum` / `maximum` (and exclusive), `minLength` /
/// `maxLength`, `minItems` / `maxItems`, `minProperties` / `maxProperties`,
/// `minContains` / `maxContains`, `multipleOf`, `const`, `enum`, `pattern`,
/// `uniqueItems`, `items`, `prefixItems`, `contains`, `propertyNames`,
/// `additionalProperties`, `formatMinimum` / `formatMaximum`, `anyOf` /
/// `oneOf` / `allOf`, `$ref`, and the `x-min*` / `x-max*` /
/// `x-exclusiveMin*` / `x-exclusiveMax*` bounds and `x-past` / `x-future`
/// flags emitted by `vld`. Stricter = breaking (see the module docs).
pub fn diff_schemas(old: &Value, new: &Value) -> SchemaDiff {
    let mut changes = Vec::new();
    diff_value(old, new, "", &mut changes);
    SchemaDiff { changes }
}

fn diff_value(old: &Value, new: &Value, path: &str, changes: &mut Vec<SchemaChange>) {
    // Nullable wrappers: compare the inner schema, report the nullability change.
    let (old, old_null) = split_nullable(old);
    let (new, new_null) = split_nullable(new);
    if old_null != new_null {
        changes.push(SchemaChange {
            path: join_path(path, "nullable"),
            kind: if new_null {
                ChangeKind::NonBreaking
            } else {
                ChangeKind::Breaking
            },
            description: if new_null {
                "Now accepts null".into()
            } else {
                "No longer accepts null".into()
            },
        });
    }

    // References: targets are not resolved, compare by name.
    if old.get("$ref") != new.get("$ref") {
        changes.push(SchemaChange {
            path: join_path(path, "$ref"),
            kind: ChangeKind::Breaking,
            description: format!(
                "Reference changed from {} to {}",
                fmt_val(old.get("$ref")),
                fmt_val(new.get("$ref"))
            ),
        });
        return;
    }

    if !diff_type(old, new, path, changes) {
        return; // incompatible types: comparing sub-properties is not meaningful
    }

    // Composition branches (`anyOf`/`oneOf` widen when branches are added,
    // `allOf` narrows).
    diff_composition(old, new, path, "anyOf", true, changes);
    diff_composition(old, new, path, "oneOf", true, changes);
    diff_composition(old, new, path, "allOf", false, changes);

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
    diff_numeric_constraint(old, new, path, "minProperties", true, changes);
    diff_numeric_constraint(old, new, path, "maxProperties", false, changes);
    diff_numeric_constraint(old, new, path, "minContains", true, changes);
    diff_numeric_constraint(old, new, path, "maxContains", false, changes);
    diff_extension_bounds(old, new, path, changes);
    diff_string_bound(old, new, path, "formatMinimum", true, changes);
    diff_string_bound(old, new, path, "formatMaximum", false, changes);
    diff_multiple_of(old, new, path, changes);
    diff_const(old, new, path, changes);
    for flag in ["uniqueItems", "x-past", "x-future"] {
        diff_flag(old, new, path, flag, changes);
    }

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
    diff_prefix_items(old, new, path, changes);
    diff_subschema(old, new, path, "contains", changes);
    diff_subschema(old, new, path, "propertyNames", changes);

    diff_additional_properties(old, new, path, changes);
}

fn change(path: &str, key: &str, kind: ChangeKind, description: String) -> SchemaChange {
    SchemaChange {
        path: join_path(path, key),
        kind,
        description,
    }
}

/// Added constraint → breaking, removed → non-breaking.
fn added_removed(
    path: &str,
    key: &str,
    old: Option<&Value>,
    new: Option<&Value>,
    changes: &mut Vec<SchemaChange>,
) -> bool {
    match (old, new) {
        (None, Some(v)) => changes.push(change(
            path,
            key,
            ChangeKind::Breaking,
            format!("{} constraint added: {}", key, v),
        )),
        (Some(v), None) => changes.push(change(
            path,
            key,
            ChangeKind::NonBreaking,
            format!("{} constraint removed (was {})", key, v),
        )),
        _ => return false,
    }
    true
}

/// A sub-schema keyword (`contains`, `propertyNames`): compared recursively.
fn diff_subschema(
    old: &Value,
    new: &Value,
    path: &str,
    key: &str,
    changes: &mut Vec<SchemaChange>,
) {
    let (o, n) = (old.get(key), new.get(key));
    if !added_removed(path, key, o, n, changes) {
        if let (Some(o), Some(n)) = (o, n) {
            diff_value(o, n, &join_path(path, key), changes);
        }
    }
}

/// Boolean constraint flags (`uniqueItems`, `x-past`, …): turning on is breaking.
fn diff_flag(old: &Value, new: &Value, path: &str, key: &str, changes: &mut Vec<SchemaChange>) {
    let on = |v: &Value| v.get(key) == Some(&Value::Bool(true));
    if on(old) != on(new) {
        changes.push(change(
            path,
            key,
            if on(new) {
                ChangeKind::Breaking
            } else {
                ChangeKind::NonBreaking
            },
            format!("{} changed from {} to {}", key, on(old), on(new)),
        ));
    }
}

/// `const`: any change narrows to a different value (breaking); removal relaxes.
fn diff_const(old: &Value, new: &Value, path: &str, changes: &mut Vec<SchemaChange>) {
    let (o, n) = (old.get("const"), new.get("const"));
    if o != n && !added_removed(path, "const", o, n, changes) {
        changes.push(change(
            path,
            "const",
            ChangeKind::Breaking,
            format!("const changed from {} to {}", fmt_val(o), fmt_val(n)),
        ));
    }
}

/// `multipleOf`: a new divisor that is a multiple of the old one (or unrelated)
/// is stricter; a divisor of the old one relaxes.
fn diff_multiple_of(old: &Value, new: &Value, path: &str, changes: &mut Vec<SchemaChange>) {
    let key = "multipleOf";
    let (o, n) = (old.get(key), new.get(key));
    if o == n || added_removed(path, key, o, n, changes) {
        return;
    }
    let (Some(a), Some(b)) = (o.and_then(Value::as_f64), n.and_then(Value::as_f64)) else {
        return;
    };
    let divides = |x: f64, y: f64| {
        let q = y / x;
        x != 0.0 && (q - q.round()).abs() <= 1e-9 * q.abs().max(1.0)
    };
    let kind = if divides(b, a) {
        ChangeKind::NonBreaking // every old multiple is still a multiple
    } else {
        ChangeKind::Breaking
    };
    changes.push(change(
        path,
        key,
        kind,
        format!("multipleOf changed from {} to {}", a, b),
    ));
}

/// `formatMinimum` / `formatMaximum`: ISO 8601 strings, compared as text.
fn diff_string_bound(
    old: &Value,
    new: &Value,
    path: &str,
    key: &str,
    is_lower_bound: bool,
    changes: &mut Vec<SchemaChange>,
) {
    let (o, n) = (old.get(key), new.get(key));
    if o == n || added_removed(path, key, o, n, changes) {
        return;
    }
    let (Some(a), Some(b)) = (o.and_then(Value::as_str), n.and_then(Value::as_str)) else {
        return;
    };
    let stricter = if is_lower_bound { b > a } else { b < a };
    changes.push(change(
        path,
        key,
        if stricter {
            ChangeKind::Breaking
        } else {
            ChangeKind::NonBreaking
        },
        format!("{} changed from {} to {}", key, a, b),
    ));
}

/// `x-min*` / `x-exclusiveMin*` (lower) and `x-max*` / `x-exclusiveMax*`
/// (upper) bounds; numbers or numeric strings (`x-minDecimal`).
fn diff_extension_bounds(old: &Value, new: &Value, path: &str, changes: &mut Vec<SchemaChange>) {
    let keys: BTreeSet<&String> = [old, new]
        .iter()
        .filter_map(|v| v.as_object())
        .flat_map(|o| o.keys())
        .filter(|k| {
            ["x-min", "x-max", "x-exclusiveMin", "x-exclusiveMax"]
                .iter()
                .any(|p| k.starts_with(p))
        })
        .collect();
    let num = |v: Option<&Value>| {
        v.and_then(|v| v.as_f64().or_else(|| v.as_str()?.parse().ok()))
    };
    for key in keys {
        let lower = key.starts_with("x-min") || key.starts_with("x-exclusiveMin");
        let (o, n) = (old.get(key), new.get(key));
        if o == n || added_removed(path, key, o, n, changes) {
            continue;
        }
        if let (Some(a), Some(b)) = (num(o), num(n)) {
            let stricter = if lower { b > a } else { b < a };
            changes.push(change(
                path,
                key,
                if stricter {
                    ChangeKind::Breaking
                } else {
                    ChangeKind::NonBreaking
                },
                format!("{} changed from {} to {}", key, a, b),
            ));
        }
    }
}

/// `prefixItems` (tuples): position by position; extra positions constrain more.
fn diff_prefix_items(old: &Value, new: &Value, path: &str, changes: &mut Vec<SchemaChange>) {
    let key = "prefixItems";
    let (o, n) = (old.get(key), new.get(key));
    if added_removed(path, key, o, n, changes) {
        return;
    }
    let (Some(o), Some(n)) = (o.and_then(Value::as_array), n.and_then(Value::as_array)) else {
        return;
    };
    let base = join_path(path, key);
    for (i, (a, b)) in o.iter().zip(n).enumerate() {
        diff_value(a, b, &format!("{}[{}]", base, i), changes);
    }
    if o.len() != n.len() {
        changes.push(SchemaChange {
            path: base,
            kind: if n.len() > o.len() {
                ChangeKind::Breaking
            } else {
                ChangeKind::NonBreaking
            },
            description: format!("prefixItems length changed from {} to {}", o.len(), n.len()),
        });
    }
}

/// `anyOf`/`oneOf` of exactly `[X, {"type": "null"}]` (either order) → `(X, true)`.
fn split_nullable(v: &Value) -> (&Value, bool) {
    for key in ["anyOf", "oneOf"] {
        if let Some([a, b]) = v.get(key).and_then(Value::as_array).map(Vec::as_slice) {
            let is_null = |x: &Value| x.get("type").and_then(Value::as_str) == Some("null");
            if is_null(b) {
                return (a, true);
            }
            if is_null(a) {
                return (b, true);
            }
        }
    }
    (v, false)
}

/// `type` as a set (`None` = unconstrained).
fn type_set(v: &Value) -> Option<BTreeSet<String>> {
    match v.get("type")? {
        Value::String(s) => Some(BTreeSet::from([s.clone()])),
        Value::Array(a) => Some(
            a.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect(),
        ),
        _ => None,
    }
}

/// Report a `type` change. Returns `false` when the types are incompatible
/// (neither accepts all values of the other).
fn diff_type(old: &Value, new: &Value, path: &str, changes: &mut Vec<SchemaChange>) -> bool {
    if old.get("type") == new.get("type") {
        return true;
    }
    let (o, n) = (type_set(old), type_set(new));
    let (widened, narrowed) = match (&o, &n) {
        (Some(o), Some(n)) => (n.is_superset(o), n.is_subset(o)),
        (Some(_), None) => (true, false),
        (None, Some(_)) => (false, true),
        (None, None) => (true, true),
    };
    changes.push(SchemaChange {
        path: join_path(path, "type"),
        kind: if widened {
            ChangeKind::NonBreaking
        } else {
            ChangeKind::Breaking
        },
        description: format!(
            "Type changed from {} to {}",
            fmt_val(old.get("type")),
            fmt_val(new.get("type"))
        ),
    });
    widened || narrowed
}

fn diff_composition(
    old: &Value,
    new: &Value,
    path: &str,
    key: &str,
    is_union: bool,
    changes: &mut Vec<SchemaChange>,
) {
    let base = join_path(path, key);
    // Adding a union branch widens; adding an `allOf` member narrows.
    let (on_add, on_remove) = if is_union {
        (ChangeKind::NonBreaking, ChangeKind::Breaking)
    } else {
        (ChangeKind::Breaking, ChangeKind::NonBreaking)
    };
    match (
        old.get(key).and_then(Value::as_array),
        new.get(key).and_then(Value::as_array),
    ) {
        (Some(o), Some(n)) => {
            for (i, (a, b)) in o.iter().zip(n).enumerate() {
                diff_value(a, b, &format!("{}[{}]", base, i), changes);
            }
            if n.len() > o.len() {
                changes.push(SchemaChange {
                    path: base,
                    kind: on_add,
                    description: format!("{} branch(es) added to {}", n.len() - o.len(), key),
                });
            } else if n.len() < o.len() {
                changes.push(SchemaChange {
                    path: base,
                    kind: on_remove,
                    description: format!("{} branch(es) removed from {}", o.len() - n.len(), key),
                });
            }
        }
        (None, Some(_)) => changes.push(SchemaChange {
            path: base,
            kind: ChangeKind::Breaking,
            description: format!("{} added", key),
        }),
        (Some(_), None) => changes.push(SchemaChange {
            path: base,
            kind: on_remove,
            description: format!("{} removed", key),
        }),
        (None, None) => {}
    }
}

fn diff_additional_properties(
    old: &Value,
    new: &Value,
    path: &str,
    changes: &mut Vec<SchemaChange>,
) {
    let (old_ap, new_ap) = (
        old.get("additionalProperties"),
        new.get("additionalProperties"),
    );
    if old_ap == new_ap {
        return;
    }
    let p = join_path(path, "additionalProperties");
    if let (Some(o @ Value::Object(_)), Some(n @ Value::Object(_))) = (old_ap, new_ap) {
        diff_value(o, n, &p, changes);
        return;
    }
    // 0 = anything (absent / true), 1 = constrained by a schema, 2 = forbidden (false)
    let level = |v: Option<&Value>| match v {
        None | Some(Value::Bool(true)) => 0,
        Some(Value::Bool(false)) => 2,
        Some(_) => 1,
    };
    let kind = if level(new_ap) > level(old_ap) {
        ChangeKind::Breaking
    } else {
        ChangeKind::NonBreaking
    };
    changes.push(SchemaChange {
        path: p,
        kind,
        description: format!(
            "additionalProperties changed from {} to {}",
            fmt_val(old_ap),
            fmt_val(new_ap)
        ),
    });
}

fn diff_required(old: &Value, new: &Value, path: &str, changes: &mut Vec<SchemaChange>) {
    let old_req = extract_string_set(old.get("required"));
    let new_req = extract_string_set(new.get("required"));
    let old_props = old.get("properties").and_then(Value::as_object);
    let new_props = new.get("properties").and_then(Value::as_object);

    for added in new_req.difference(&old_req) {
        // A brand-new required property is reported once, by `diff_properties`.
        let is_new_prop = matches!((old_props, new_props), (Some(o), Some(n))
            if !o.contains_key(added.as_str()) && n.contains_key(added.as_str()));
        if is_new_prop {
            continue;
        }
        changes.push(SchemaChange {
            path: join_path(path, &format!("required[{}]", added)),
            kind: ChangeKind::Breaking,
            description: format!("Field \"{}\" is now required", added),
        });
    }

    for removed in old_req.difference(&new_req) {
        // A removed property is reported by `diff_properties`.
        let prop_removed = new_props
            .map(|p| !p.contains_key(removed.as_str()))
            .unwrap_or(false);
        if !prop_removed {
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
