//! The schemars bridge, compiled once per major by `schemars0.rs` /
//! `schemars1.rs` against their `schemars` alias. The version-independent
//! parts (validator, introspection, `to_openapi3`) live in
//! [`crate::validator`] and are re-exported here.

use super::schemars;
use crate::validator::rewrite_refs;
use serde_json::Value;

pub use crate::validator::{
    PropertyInfo, VldSchemarsError, get_property, is_required, list_properties,
    overlay_constraints, schema_type, schemas_equal, to_openapi3, validate_with_schema,
};

// ========================= vld → schemars ====================================

/// Convert a `serde_json::Value` (JSON Schema produced by `vld`) into a
/// `schemars::Schema`.
///
/// A value that is not a JSON Schema (neither an object nor a boolean) fails
/// closed: it becomes the `false` schema, which rejects everything. Use
/// [`try_vld_to_schemars`] to get an error instead.
pub fn vld_to_schemars(value: &Value) -> schemars::Schema {
    try_vld_to_schemars(value).unwrap_or_else(|_| schemars::Schema::from(false))
}

/// Like [`vld_to_schemars`], but a value that is not a JSON Schema (neither an
/// object nor a boolean) is an error.
pub fn try_vld_to_schemars(value: &Value) -> Result<schemars::Schema, VldSchemarsError> {
    value.clone().try_into().map_err(|_| {
        VldSchemarsError::Deserialization(format!(
            "not a JSON Schema (expected an object or a boolean): {value}"
        ))
    })
}

/// Generate a `schemars::Schema` from the `json_schema()` of a vld schema or
/// type (`vld::schema!` with the `openapi` feature, `#[derive(Validate)]`).
pub fn vld_schema_to_schemars(vld_json: &Value) -> schemars::Schema {
    vld_to_schemars(vld_json)
}

// ========================= schemars → vld ====================================

/// Convert a `schemars::Schema` to a `serde_json::Value`: a standard JSON
/// Schema for documentation, comparison or other tools.
pub fn schemars_to_json(schema: &schemars::Schema) -> Value {
    schema.as_value().clone()
}

/// Generate a vld-compatible JSON Schema value from a type implementing
/// `schemars::JsonSchema`.
pub fn generate_from_schemars<T: schemars::JsonSchema>() -> Value {
    let schema = schemars::SchemaGenerator::default().into_root_schema_for::<T>();
    schemars_to_json(&schema)
}

/// Generate a root `schemars::Schema` for a type implementing `schemars::JsonSchema`.
///
/// Convenience wrapper around `SchemaGenerator::into_root_schema_for`.
pub fn generate_schemars<T: schemars::JsonSchema>() -> schemars::Schema {
    schemars::SchemaGenerator::default().into_root_schema_for::<T>()
}

// ========================= schemars → vld validation =========================

/// Validate a `serde_json::Value` against a `schemars::Schema`
/// ([`validate_with_schema`] on its JSON).
pub fn validate_with_schemars(
    schema: &schemars::Schema,
    value: &Value,
) -> Result<(), vld::error::VldError> {
    validate_with_schema(schema.as_value(), value)
}

/// Trait for types that can be validated using their `schemars::JsonSchema`.
///
/// Implemented by [`impl_vld_parse!`](super::impl_vld_parse).
pub trait SchemarsValidate: schemars::JsonSchema + serde::Serialize + Sized {
    /// Validate this value against its schemars-generated JSON Schema.
    fn vld_validate(&self) -> Result<(), vld::error::VldError> {
        let schema = generate_schemars::<Self>();
        let json = serde_json::to_value(self).map_err(|e| {
            vld::error::VldError::single(
                vld::error::IssueCode::ParseError,
                format!("Serialization error: {}", e),
            )
        })?;
        validate_with_schemars(&schema, &json)
    }

    /// Validate a JSON value against this type's schemars schema.
    fn vld_validate_json(value: &Value) -> Result<(), vld::error::VldError> {
        let schema = generate_schemars::<Self>();
        validate_with_schemars(&schema, value)
    }

    /// Validate and deserialize a JSON value into this type.
    fn vld_parse(value: &Value) -> Result<Self, VldSchemarsError>
    where
        Self: serde::de::DeserializeOwned,
    {
        Self::vld_validate_json(value).map_err(VldSchemarsError::Validation)?;
        serde_json::from_value(value.clone())
            .map_err(|e| VldSchemarsError::Deserialization(e.to_string()))
    }
}

/// Extract property info from a `schemars::Schema` ([`list_properties`] on its JSON).
pub fn list_properties_schemars(schema: &schemars::Schema) -> Vec<PropertyInfo> {
    list_properties(schema.as_value())
}

/// Merge two `schemars::Schema` into one using `allOf`.
pub fn merge_schemas(a: &schemars::Schema, b: &schemars::Schema) -> schemars::Schema {
    let merged = serde_json::json!({
        "allOf": [a.as_value(), b.as_value()]
    });
    vld_to_schemars(&merged)
}

// ========================= impl_json_schema! =================================

/// Register nested vld schemas in the generator and rewrite vld's
/// `#/components/schemas/X` refs to the generator's definitions path.
#[doc(hidden)]
pub fn __vld_json_schema(
    generator: &mut schemars::SchemaGenerator,
    mut root: Value,
    nested: Vec<vld::openapi::NestedSchemaEntry>,
) -> schemars::Schema {
    let path = &generator.settings().definitions_path;
    let path = path.strip_prefix('#').unwrap_or(path);
    let prefix = format!("#{}/", path.strip_suffix('/').unwrap_or(path));
    let oas3 = generator
        .settings()
        .meta_schema
        .as_deref()
        .is_some_and(|m| m.contains("/oas/3.0"));
    if oas3 {
        to_openapi3(&mut root);
    }
    for (name, schema_fn) in nested {
        let mut schema = schema_fn();
        if oas3 {
            to_openapi3(&mut schema);
        }
        rewrite_refs(&mut schema, &prefix);
        generator
            .definitions_mut()
            .entry(name.to_string())
            .or_insert(schema);
    }
    rewrite_refs(&mut root, &prefix);
    vld_to_schemars(&root)
}

// ========================= Prelude ===========================================

/// Common imports: the conversion functions and macros plus `vld::prelude`.
pub mod prelude {
    pub use super::super::{impl_json_schema, impl_vld_parse};
    pub use super::{
        PropertyInfo, SchemarsValidate, VldSchemarsError, generate_from_schemars,
        generate_schemars, get_property, is_required, list_properties, list_properties_schemars,
        merge_schemas, overlay_constraints, schema_type, schemars_to_json, schemas_equal,
        try_vld_to_schemars, validate_with_schema, validate_with_schemars, vld_schema_to_schemars,
        vld_to_schemars,
    };
    pub use vld::prelude::*;
}
