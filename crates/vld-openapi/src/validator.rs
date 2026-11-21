//! Version-independent JSON Schema tools: validate a JSON value against a
//! JSON Schema, inspect and combine schemas, rewrite a 2020-12 schema into the
//! OpenAPI 3.0 dialect. No schemars dependency; the schemars modules
//! re-export every item.
//!
//! ```rust
//! use vld_openapi::validator::*;
//!
//! let schema = serde_json::json!({
//!     "type": "object",
//!     "required": ["name"],
//!     "properties": { "name": { "type": "string", "minLength": 1 } }
//! });
//! assert!(validate_with_schema(&schema, &serde_json::json!({"name": "Alice"})).is_ok());
//! assert!(is_required(&schema, "name"));
//! assert_eq!(list_properties(&schema)[0].name, "name");
//! ```

use serde_json::Value;
use std::fmt;

mod engine;

// ========================= Error type ========================================

/// Error returned by schemars → vld validation.
#[derive(Debug, Clone)]
pub enum VldSchemarsError {
    /// JSON Schema validation failed.
    Validation(vld::error::VldError),
    /// Deserialization failed.
    Deserialization(String),
}

impl fmt::Display for VldSchemarsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VldSchemarsError::Validation(e) => write!(f, "Schema validation error: {}", e),
            VldSchemarsError::Deserialization(e) => write!(f, "Deserialization error: {}", e),
        }
    }
}

impl std::error::Error for VldSchemarsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VldSchemarsError::Validation(e) => Some(e),
            VldSchemarsError::Deserialization(_) => None,
        }
    }
}

impl From<vld::error::VldError> for VldSchemarsError {
    fn from(e: vld::error::VldError) -> Self {
        VldSchemarsError::Validation(e)
    }
}

// ========================= Validation ========================================

/// Validate a `serde_json::Value` against a JSON Schema.
///
/// Performs structural validation: type checks, required fields,
/// string constraints (minLength, maxLength, pattern, format),
/// number constraints (minimum, maximum, exclusiveMinimum, exclusiveMaximum,
/// multipleOf — exact for integers), array constraints (minItems, maxItems,
/// uniqueItems, contains / minContains / maxContains), `propertyNames`,
/// minProperties / maxProperties, `formatMinimum` / `formatMaximum` for
/// `date` / `date-time` / `time`, enum values, `oneOf` / `anyOf` / `allOf` /
/// `not`, and recursive validation of `properties`, `patternProperties`,
/// `additionalProperties`, `items` / `prefixItems`, and local `$ref`s
/// (resolved against `schema`).
///
/// Not checked: vendor `x-*` keywords (vld's `x-past` / `x-future`,
/// `x-minUniqueItems` / `x-maxUniqueItems`, `x-minDecimal`, …), and most
/// `format`s beyond a basic shape check (unknown formats pass).
///
/// ```rust
/// let schema = serde_json::json!({
///     "type": "object",
///     "required": ["name"],
///     "properties": {
///         "name": { "type": "string", "minLength": 1 },
///         "age":  { "type": "integer", "minimum": 0 }
///     }
/// });
/// let valid = serde_json::json!({"name": "Alice", "age": 30});
/// assert!(vld_openapi::validator::validate_with_schema(&schema, &valid).is_ok());
///
/// let invalid = serde_json::json!({"age": -5});
/// assert!(vld_openapi::validator::validate_with_schema(&schema, &invalid).is_err());
/// ```
pub fn validate_with_schema(schema: &Value, value: &Value) -> Result<(), vld::error::VldError> {
    engine::validate_value_against_schema(
        schema,
        value,
        &[],
        engine::Ctx {
            root: schema,
            depth: 0,
        },
    )
}

// ========================= Introspection =====================================

/// Information about a single property in a JSON Schema object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyInfo {
    /// Property name.
    pub name: String,
    /// JSON Schema "type" value (e.g. "string", "integer", "number", "boolean", "object", "array").
    pub schema_type: Option<String>,
    /// Whether this property is listed in "required".
    pub required: bool,
    /// The raw JSON schema for this property.
    pub schema: Value,
}

/// Extract property information from a JSON Schema object, sorted by name.
///
/// Works with both `schemars::Schema` (via `as_value()`) and raw `serde_json::Value`.
///
/// ```rust
/// use vld::prelude::*;
///
/// vld::schema! {
///     #[derive(Debug)]
///     pub struct UserSchema {
///         pub name: String => vld::string().min(1),
///         pub age: i64 => vld::number().int().min(0),
///     }
/// }
///
/// let json = UserSchema::json_schema();
/// let props = vld_openapi::validator::list_properties(&json);
/// assert_eq!(props.len(), 2);
/// ```
pub fn list_properties(schema: &Value) -> Vec<PropertyInfo> {
    let mut result = Vec::new();

    let properties = match schema.get("properties").and_then(|p| p.as_object()) {
        Some(p) => p,
        None => return result,
    };

    let required_set: std::collections::HashSet<&str> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    for (name, prop_schema) in properties {
        let schema_type = prop_schema
            .get("type")
            .and_then(|t| t.as_str())
            .map(String::from);

        result.push(PropertyInfo {
            name: name.clone(),
            schema_type,
            required: required_set.contains(name.as_str()),
            schema: prop_schema.clone(),
        });
    }

    // Name order whatever map order serde_json uses (`preserve_order`).
    result.sort_unstable_by(|a, b| a.name.cmp(&b.name));
    result
}

/// Get the "type" field from a JSON Schema.
///
/// ```rust
/// let schema = serde_json::json!({"type": "string"});
/// assert_eq!(vld_openapi::validator::schema_type(&schema), Some("string".to_string()));
/// ```
pub fn schema_type(schema: &Value) -> Option<String> {
    schema
        .get("type")
        .and_then(|t| t.as_str())
        .map(String::from)
}

/// Check if a field is required in a JSON Schema.
///
/// ```rust
/// let schema = serde_json::json!({
///     "type": "object",
///     "required": ["name"],
///     "properties": { "name": { "type": "string" } }
/// });
/// assert!(vld_openapi::validator::is_required(&schema, "name"));
/// assert!(!vld_openapi::validator::is_required(&schema, "age"));
/// ```
pub fn is_required(schema: &Value, field: &str) -> bool {
    schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().any(|v| v.as_str() == Some(field)))
        .unwrap_or(false)
}

/// Get the property schema for a specific field.
///
/// ```rust
/// let schema = serde_json::json!({
///     "type": "object",
///     "properties": { "name": { "type": "string", "minLength": 1 } }
/// });
/// let name_schema = vld_openapi::validator::get_property(&schema, "name").unwrap();
/// assert_eq!(name_schema["type"], "string");
/// ```
pub fn get_property<'a>(schema: &'a Value, field: &str) -> Option<&'a Value> {
    schema
        .get("properties")
        .and_then(|p| p.as_object())
        .and_then(|props| props.get(field))
}

// ========================= Comparison & Merge ================================

/// Check if two JSON Schema values are structurally equal.
///
/// ```rust
/// use vld::prelude::*;
/// use vld::openapi::JsonSchema;
///
/// let a = vld::string().min(1).json_schema();
/// let b = serde_json::json!({"type": "string", "minLength": 1});
/// assert!(vld_openapi::validator::schemas_equal(&a, &b));
/// ```
pub fn schemas_equal(a: &Value, b: &Value) -> bool {
    a == b
}

/// Overlay additional constraints from one schema onto another.
///
/// Copies `properties`, `required`, and validation keywords (`minLength`,
/// `maxLength`, `minimum`, `maximum`, `pattern`, `format`) from `overlay`
/// into a clone of `base`. Existing `base` values are preserved.
///
/// ```rust
/// let base = serde_json::json!({"type": "object", "properties": {"name": {"type": "string"}}});
/// let overlay = serde_json::json!({"properties": {"name": {"minLength": 2}}, "required": ["name"]});
/// let result = vld_openapi::validator::overlay_constraints(&base, &overlay);
/// assert!(vld_openapi::validator::is_required(&result, "name"));
/// ```
pub fn overlay_constraints(base: &Value, overlay: &Value) -> Value {
    let mut result = base.clone();

    if let (Some(result_obj), Some(overlay_obj)) = (result.as_object_mut(), overlay.as_object()) {
        for (key, value) in overlay_obj {
            match key.as_str() {
                "properties" => {
                    if let (Some(base_props), Some(overlay_props)) = (
                        result_obj
                            .get_mut("properties")
                            .and_then(|p| p.as_object_mut()),
                        value.as_object(),
                    ) {
                        for (prop_name, prop_schema) in overlay_props {
                            if let Some(existing) = base_props.get_mut(prop_name) {
                                if let (Some(existing_obj), Some(overlay_prop_obj)) =
                                    (existing.as_object_mut(), prop_schema.as_object())
                                {
                                    for (k, v) in overlay_prop_obj {
                                        existing_obj.entry(k.clone()).or_insert_with(|| v.clone());
                                    }
                                }
                            } else {
                                base_props.insert(prop_name.clone(), prop_schema.clone());
                            }
                        }
                    }
                }
                "required" => {
                    if let Some(overlay_required) = value.as_array() {
                        let base_required = result_obj
                            .entry("required")
                            .or_insert_with(|| Value::Array(vec![]));
                        if let Some(arr) = base_required.as_array_mut() {
                            for item in overlay_required {
                                if !arr.contains(item) {
                                    arr.push(item.clone());
                                }
                            }
                        }
                    }
                }
                _ => {
                    result_obj
                        .entry(key.clone())
                        .or_insert_with(|| value.clone());
                }
            }
        }
    }

    result
}

// ========================= OpenAPI 3.0 =======================================

/// Rewrite vld's JSON Schema (2020-12) into the OpenAPI 3.0 dialect, applied
/// when the generator uses `SchemaSettings::openapi3()`:
///
/// - tuples (`prefixItems` + `items: false`) become `items: {anyOf: [...]}`
///   with the existing `minItems` / `maxItems`;
/// - `null` branches of `anyOf` / `oneOf` and `"null"` in a `type` array
///   become `nullable: true` (a lone remaining `$ref` is wrapped in `allOf`);
/// - numeric `exclusiveMinimum` / `exclusiveMaximum` (2020-12) become
///   `minimum` / `maximum` plus the boolean 3.0 `exclusiveMinimum: true` /
///   `exclusiveMaximum: true`;
/// - a schema that only allows `null` becomes
///   `{"type": "string", "nullable": true, "enum": [null]}` (3.0 ignores
///   `nullable` without a `type`, and has no `null` type).
///
/// Doing it here keeps schemars' own OpenAPI 3.0 transforms from producing
/// `additionalItems` or `{"enum": [null], "nullable": true}`.
pub fn to_openapi3(value: &mut Value) {
    match value {
        Value::Array(items) => items.iter_mut().for_each(to_openapi3),
        Value::Object(map) => {
            if let Some(Value::Array(prefix)) = map.remove("prefixItems") {
                let items = match <[Value; 1]>::try_from(prefix) {
                    Ok([one]) => one,
                    Err(many) => serde_json::json!({ "anyOf": many }),
                };
                map.insert("items".into(), items);
            }

            if let Some(Value::Array(types)) = map.get("type") {
                let rest: Vec<Value> = types.iter().filter(|t| *t != "null").cloned().collect();
                if rest.len() < types.len() {
                    map.insert("nullable".into(), Value::Bool(true));
                    match <[Value; 1]>::try_from(rest) {
                        Ok([one]) => map.insert("type".into(), one),
                        // `["null"]`: a null-only schema, rewritten below.
                        Err(rest) if rest.is_empty() => map.insert("type".into(), "null".into()),
                        Err(rest) => map.insert("type".into(), Value::Array(rest)),
                    };
                }
            }

            for key in ["anyOf", "oneOf"] {
                let Some(Value::Array(branches)) = map.get(key) else {
                    continue;
                };
                let rest: Vec<Value> = branches
                    .iter()
                    .filter(|b| !is_null_schema(b))
                    .cloned()
                    .collect();
                if rest.len() == branches.len() {
                    continue;
                }
                map.remove(key);
                map.insert("nullable".into(), Value::Bool(true));
                match <[Value; 1]>::try_from(rest) {
                    Ok([Value::Object(one)]) if !one.contains_key("$ref") => {
                        for (k, v) in one {
                            map.entry(k).or_insert(v);
                        }
                    }
                    Ok([one]) => {
                        map.insert("allOf".into(), Value::Array(vec![one]));
                    }
                    // Only `null` branches: a null-only schema, rewritten below.
                    Err(rest) if rest.is_empty() => {
                        map.insert("type".into(), "null".into());
                    }
                    Err(rest) => {
                        map.insert(key.into(), Value::Array(rest));
                    }
                }
            }

            // 2020-12 numeric `exclusiveMinimum: n` → 3.0 `minimum: n, exclusiveMinimum: true`
            // (an inclusive bound that is already stricter wins).
            for (ex, inc, stricter) in [
                ("exclusiveMinimum", "minimum", std::cmp::Ordering::Greater),
                ("exclusiveMaximum", "maximum", std::cmp::Ordering::Less),
            ] {
                let Some(n) = map.get(ex).and_then(Value::as_f64) else {
                    continue;
                };
                let n_value = map.remove(ex).unwrap_or_default();
                let inclusive_wins = map
                    .get(inc)
                    .and_then(Value::as_f64)
                    .and_then(|m| m.partial_cmp(&n))
                    .is_some_and(|o| o == stricter);
                if !inclusive_wins {
                    map.insert(inc.into(), n_value);
                    map.insert(ex.into(), Value::Bool(true));
                }
            }

            // `nullable` needs a `type` in 3.0: a null-only schema becomes a
            // nullable string whose only allowed value is `null`.
            if is_null_schema(&Value::Object(map.clone())) {
                for k in ["type", "const", "enum", "nullable"] {
                    map.remove(k);
                }
                map.insert("type".into(), "string".into());
                map.insert("nullable".into(), Value::Bool(true));
                map.insert("enum".into(), serde_json::json!([null]));
                return;
            }
            // Children last: null branches must still look like `null` above.
            map.values_mut().for_each(to_openapi3);
        }
        _ => {}
    }
}

/// `{"type": "null"}`, `{"const": null}` or `{"enum": [null]}`.
fn is_null_schema(v: &Value) -> bool {
    v.get("type").is_some_and(|t| t == "null")
        || v.get("const").is_some_and(Value::is_null)
        || v.get("enum") == Some(&serde_json::json!([null]))
}

/// Point local refs (`#/components/schemas/X`, `#/$defs/X`, `#/definitions/X`)
/// at `prefix` (e.g. `#/$defs/`).
#[cfg(any(feature = "schemars-0", feature = "schemars-1"))]
pub(crate) fn rewrite_refs(value: &mut Value, prefix: &str) {
    match value {
        Value::Object(map) => {
            for (key, v) in map.iter_mut() {
                match v {
                    Value::String(r) if key == "$ref" => {
                        let name = ["#/components/schemas/", "#/$defs/", "#/definitions/"]
                            .iter()
                            .find_map(|p| r.strip_prefix(p));
                        if let Some(name) = name {
                            *r = format!("{prefix}{name}");
                        }
                    }
                    _ => rewrite_refs(v, prefix),
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|v| rewrite_refs(v, prefix)),
        _ => {}
    }
}
