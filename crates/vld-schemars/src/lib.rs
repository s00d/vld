//! # vld-schemars — Bidirectional bridge between `vld` and `schemars`
//!
//! Many Rust libraries (aide, paperclip, okapi, utoipa-rapidoc, etc.) already use
//! [`schemars`](https://docs.rs/schemars) for JSON Schema generation. This crate
//! lets you share schema definitions between `vld` and the broader `schemars`
//! ecosystem — in **both** directions.
//!
//! ## vld → schemars
//!
//! ```rust
//! use vld::prelude::*;
//! use vld_schemars::impl_json_schema;
//!
//! vld::schema! {
//!     #[derive(Debug)]
//!     pub struct User {
//!         pub name: String => vld::string().min(2).max(50),
//!         pub email: String => vld::string().email(),
//!     }
//! }
//!
//! impl_json_schema!(User);
//! // User now implements schemars::JsonSchema
//! ```
//!
//! ## schemars → vld (validation)
//!
//! ```rust
//! // Validate data using a JSON Schema from schemars
//! let schema = serde_json::json!({
//!     "type": "object",
//!     "required": ["name"],
//!     "properties": { "name": {"type": "string", "minLength": 1} }
//! });
//! let data = serde_json::json!({"name": "Alice"});
//! assert!(vld_schemars::validate_with_schema(&schema, &data).is_ok());
//! ```
//!
//! ```rust
//! // Implement vld::VldParse for a schemars type (reverse of impl_json_schema!)
//! use vld_schemars::impl_vld_parse;
//!
//! #[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
//! struct Item { name: String, qty: u32 }
//! impl_vld_parse!(Item);
//!
//! // Now Item::vld_parse_value() validates using schemars-generated schema
//! ```

use serde_json::Value;
use std::fmt;

pub use schemars;
pub use vld;

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

// ========================= vld → schemars ====================================

/// Convert a `serde_json::Value` (JSON Schema produced by `vld`) into a
/// `schemars::Schema`.
///
/// ```rust
/// let schema = vld_schemars::vld_to_schemars(&serde_json::json!({"type": "string"}));
/// assert_eq!(schema.get("type").unwrap(), "string");
/// ```
pub fn vld_to_schemars(value: &Value) -> schemars::Schema {
    value
        .clone()
        .try_into()
        .unwrap_or_else(|_| schemars::Schema::default())
}

/// Generate a `schemars::Schema` from a vld type that has a `json_schema()` method.
///
/// Works with types defined via `vld::schema!` (with `openapi` feature) or `#[derive(Validate)]`.
///
/// ```rust
/// use vld::prelude::*;
/// use vld::json_schema::JsonSchema;
///
/// let schema = vld_schemars::vld_schema_to_schemars(&vld::string().email().json_schema());
/// assert_eq!(schema.get("type").unwrap(), "string");
/// assert_eq!(schema.get("format").unwrap(), "email");
/// ```
pub fn vld_schema_to_schemars(vld_json: &Value) -> schemars::Schema {
    vld_to_schemars(vld_json)
}

// ========================= schemars → vld ====================================

/// Convert a `schemars::Schema` to a `serde_json::Value`.
///
/// The resulting JSON value is a standard JSON Schema that can be used for
/// documentation, comparison, or feeding into other tools.
///
/// ```rust
/// use schemars::JsonSchema;
///
/// let schemars_schema = schemars::SchemaGenerator::default().into_root_schema_for::<String>();
/// let json = vld_schemars::schemars_to_json(&schemars_schema);
/// assert!(json.is_object());
/// ```
pub fn schemars_to_json(schema: &schemars::Schema) -> Value {
    schema.as_value().clone()
}

/// Generate a vld-compatible JSON Schema value from a type implementing
/// `schemars::JsonSchema`.
///
/// ```rust
/// let schema = vld_schemars::generate_from_schemars::<String>();
/// assert_eq!(schema["type"], "string");
/// ```
pub fn generate_from_schemars<T: schemars::JsonSchema>() -> Value {
    let schema = schemars::SchemaGenerator::default().into_root_schema_for::<T>();
    schemars_to_json(&schema)
}

/// Generate a root `schemars::Schema` for a type implementing `schemars::JsonSchema`.
///
/// Convenience wrapper around `SchemaGenerator::into_root_schema_for`.
///
/// ```rust
/// let schema = vld_schemars::generate_schemars::<i32>();
/// assert!(schema.get("type").is_some());
/// ```
pub fn generate_schemars<T: schemars::JsonSchema>() -> schemars::Schema {
    schemars::SchemaGenerator::default().into_root_schema_for::<T>()
}

// ========================= schemars → vld validation =========================

mod validator;

/// Validate a `serde_json::Value` against a JSON Schema.
///
/// Performs structural validation: type checks, required fields,
/// string constraints (minLength, maxLength, pattern, format),
/// number constraints (minimum, maximum, exclusiveMinimum, exclusiveMaximum),
/// array constraints (minItems, maxItems), enum values, and recursive
/// validation of `properties` and `items`.
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
/// assert!(vld_schemars::validate_with_schema(&schema, &valid).is_ok());
///
/// let invalid = serde_json::json!({"age": -5});
/// assert!(vld_schemars::validate_with_schema(&schema, &invalid).is_err());
/// ```
pub fn validate_with_schema(
    schema: &Value,
    value: &Value,
) -> Result<(), vld::error::VldError> {
    validator::validate_value_against_schema(schema, value, &[])
}

/// Validate a `serde_json::Value` against a `schemars::Schema`.
///
/// ```rust
/// let schema = vld_schemars::vld_to_schemars(&serde_json::json!({
///     "type": "string", "minLength": 2
/// }));
/// assert!(vld_schemars::validate_with_schemars(&schema, &serde_json::json!("hello")).is_ok());
/// assert!(vld_schemars::validate_with_schemars(&schema, &serde_json::json!("x")).is_err());
/// ```
pub fn validate_with_schemars(
    schema: &schemars::Schema,
    value: &Value,
) -> Result<(), vld::error::VldError> {
    validate_with_schema(schema.as_value(), value)
}

/// Validate a serializable value against a `schemars::Schema`.
///
/// ```rust
/// let schema = vld_schemars::vld_to_schemars(&serde_json::json!({
///     "type": "object",
///     "required": ["name"],
///     "properties": { "name": {"type": "string", "minLength": 1} }
/// }));
///
/// #[derive(serde::Serialize)]
/// struct User { name: String }
///
/// let user = User { name: "Alice".into() };
/// assert!(vld_schemars::validate_serde_with_schemars(&schema, &user).is_ok());
/// ```
pub fn validate_serde_with_schemars<T: serde::Serialize>(
    schema: &schemars::Schema,
    value: &T,
) -> Result<(), VldSchemarsError> {
    let json = serde_json::to_value(value)
        .map_err(|e| VldSchemarsError::Deserialization(e.to_string()))?;
    validate_with_schemars(schema, &json).map_err(VldSchemarsError::Validation)
}

/// Validate and deserialize a JSON value using a type's `schemars::JsonSchema`.
///
/// Generates the schema from `T`, validates `value` against it, then
/// deserializes into `T`.
///
/// ```rust
/// let json = serde_json::json!("hello world");
/// let result: String = vld_schemars::parse_with_schemars::<String>(&json).unwrap();
/// assert_eq!(result, "hello world");
/// ```
pub fn parse_with_schemars<T: schemars::JsonSchema + serde::de::DeserializeOwned>(
    value: &Value,
) -> Result<T, VldSchemarsError> {
    let schema = generate_schemars::<T>();
    validate_with_schemars(&schema, value).map_err(VldSchemarsError::Validation)?;
    serde_json::from_value(value.clone())
        .map_err(|e| VldSchemarsError::Deserialization(e.to_string()))
}

/// Implement `vld::schema::VldParse` for a type that implements
/// `schemars::JsonSchema + serde::de::DeserializeOwned`.
///
/// This is the **reverse** of [`impl_json_schema!`]: given a schemars type,
/// make it usable with vld's validation framework (extractors, etc.).
///
/// ```rust
/// use vld_schemars::impl_vld_parse;
///
/// #[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
/// struct Item {
///     name: String,
///     qty: u32,
/// }
///
/// impl_vld_parse!(Item);
///
/// // Now Item implements vld::schema::VldParse
/// use vld::schema::VldParse;
/// let json = serde_json::json!({"name": "Widget", "qty": 5});
/// let item = Item::vld_parse_value(&json).unwrap();
/// assert_eq!(item.name, "Widget");
/// ```
#[macro_export]
macro_rules! impl_vld_parse {
    ($ty:ty) => {
        impl $crate::vld::schema::VldParse for $ty {
            fn vld_parse_value(
                value: &::serde_json::Value,
            ) -> ::std::result::Result<Self, $crate::vld::error::VldError> {
                let schema = $crate::generate_schemars::<$ty>();
                $crate::validate_with_schemars(&schema, value)?;
                ::serde_json::from_value(value.clone()).map_err(|e| {
                    $crate::vld::error::VldError::single(
                        $crate::vld::error::IssueCode::ParseError,
                        format!("Deserialization error: {}", e),
                    )
                })
            }
        }
    };
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

/// Extract property information from a JSON Schema object.
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
/// let props = vld_schemars::list_properties(&json);
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

    result
}

/// Extract property info from a `schemars::Schema`.
pub fn list_properties_schemars(schema: &schemars::Schema) -> Vec<PropertyInfo> {
    list_properties(schema.as_value())
}

/// Get the "type" field from a JSON Schema.
///
/// ```rust
/// let schema = serde_json::json!({"type": "string"});
/// assert_eq!(vld_schemars::schema_type(&schema), Some("string".to_string()));
/// ```
pub fn schema_type(schema: &Value) -> Option<String> {
    schema.get("type").and_then(|t| t.as_str()).map(String::from)
}

/// Check if a field is required in a JSON Schema.
///
/// ```rust
/// let schema = serde_json::json!({
///     "type": "object",
///     "required": ["name"],
///     "properties": { "name": { "type": "string" } }
/// });
/// assert!(vld_schemars::is_required(&schema, "name"));
/// assert!(!vld_schemars::is_required(&schema, "age"));
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
/// let name_schema = vld_schemars::get_property(&schema, "name").unwrap();
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
/// use vld::json_schema::JsonSchema;
///
/// let a = vld::string().min(1).json_schema();
/// let b = serde_json::json!({"type": "string", "minLength": 1});
/// assert!(vld_schemars::schemas_equal(&a, &b));
/// ```
pub fn schemas_equal(a: &Value, b: &Value) -> bool {
    a == b
}

/// Merge two `schemars::Schema` into one using `allOf`.
///
/// ```rust
/// let a = vld_schemars::vld_to_schemars(&serde_json::json!({"type": "object", "properties": {"name": {"type": "string"}}}));
/// let b = vld_schemars::vld_to_schemars(&serde_json::json!({"type": "object", "properties": {"age": {"type": "integer"}}}));
/// let merged = vld_schemars::merge_schemas(&a, &b);
/// assert!(merged.get("allOf").is_some());
/// ```
pub fn merge_schemas(a: &schemars::Schema, b: &schemars::Schema) -> schemars::Schema {
    let merged = serde_json::json!({
        "allOf": [a.as_value(), b.as_value()]
    });
    vld_to_schemars(&merged)
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
/// let result = vld_schemars::overlay_constraints(&base, &overlay);
/// assert!(vld_schemars::is_required(&result, "name"));
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
                    result_obj.entry(key.clone()).or_insert_with(|| value.clone());
                }
            }
        }
    }

    result
}

// ========================= impl_json_schema! =================================

/// Implement `schemars::JsonSchema` for a type that has a `json_schema()`
/// associated function (generated by `vld::schema!` or `#[derive(Validate)]`).
///
/// # Usage
///
/// ```rust
/// use vld::prelude::*;
/// use vld_schemars::impl_json_schema;
///
/// vld::schema! {
///     #[derive(Debug)]
///     pub struct CreateUser {
///         pub name: String => vld::string().min(2).max(100),
///         pub email: String => vld::string().email(),
///     }
/// }
///
/// impl_json_schema!(CreateUser);
///
/// // Now CreateUser implements schemars::JsonSchema
/// ```
///
/// With a custom schema name:
///
/// ```rust
/// # use vld::prelude::*;
/// # use vld_schemars::impl_json_schema;
/// # vld::schema! {
/// #     #[derive(Debug)]
/// #     pub struct Req { pub x: String => vld::string() }
/// # }
/// impl_json_schema!(Req, "CreateUserRequest");
/// ```
#[macro_export]
macro_rules! impl_json_schema {
    ($ty:ty) => {
        impl $crate::schemars::JsonSchema for $ty {
            fn schema_name() -> ::std::borrow::Cow<'static, str> {
                ::std::borrow::Cow::Borrowed(stringify!($ty))
            }

            fn schema_id() -> ::std::borrow::Cow<'static, str> {
                ::std::borrow::Cow::Owned(concat!(module_path!(), "::", stringify!($ty)).to_owned())
            }

            fn json_schema(
                _gen: &mut $crate::schemars::SchemaGenerator,
            ) -> $crate::schemars::Schema {
                $crate::vld_to_schemars(&<$ty>::json_schema())
            }
        }
    };
    ($ty:ty, $name:expr) => {
        impl $crate::schemars::JsonSchema for $ty {
            fn schema_name() -> ::std::borrow::Cow<'static, str> {
                ::std::borrow::Cow::Borrowed($name)
            }

            fn schema_id() -> ::std::borrow::Cow<'static, str> {
                ::std::borrow::Cow::Owned(format!("{}::{}", module_path!(), $name))
            }

            fn json_schema(
                _gen: &mut $crate::schemars::SchemaGenerator,
            ) -> $crate::schemars::Schema {
                $crate::vld_to_schemars(&<$ty>::json_schema())
            }
        }
    };
}

// ========================= Prelude ===========================================

pub mod prelude {
    pub use crate::impl_json_schema;
    pub use crate::impl_vld_parse;
    pub use crate::{
        generate_from_schemars, generate_schemars, get_property, is_required, list_properties,
        list_properties_schemars, merge_schemas, overlay_constraints, parse_with_schemars,
        schema_type, schemas_equal, schemars_to_json, validate_serde_with_schemars,
        validate_with_schema, validate_with_schemars, vld_schema_to_schemars, vld_to_schemars,
        PropertyInfo, VldSchemarsError,
    };
    pub use vld::prelude::*;
}
