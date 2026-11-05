//! JSON Schema / OpenAPI generation from `vld` schemas.
//!
//! The [`JsonSchema`] trait is implemented by schema types that can produce a
//! [JSON Schema](https://json-schema.org/) representation. The output is
//! compatible with **OpenAPI 3.1** (which uses JSON Schema directly).
//!
//! Nullable/optional values and unions are emitted as `anyOf` (not `oneOf`):
//! `vld` unions are first-match, and an inner schema that already accepts
//! `null` would make a `oneOf` with `{"type": "null"}` unsatisfiable.
//!
//! # Example
//!
//! ```
//! use vld::prelude::*;
//! use vld::openapi::JsonSchema;
//!
//! let schema = vld::string().min(2).max(50).email();
//! let js = schema.json_schema();
//! assert_eq!(js["type"], "string");
//! assert_eq!(js["minLength"], 2);
//! assert_eq!(js["format"], "email");
//! ```

use serde_json::{json, Value};

#[cfg(feature = "diff")]
pub mod diff;

pub use crate::schema::NestedSchemaEntry;

/// Trait for types that can produce a JSON Schema representation.
///
/// Implemented for all core `vld` schemas. The output is a `serde_json::Value`
/// representing the JSON Schema object, compatible with OpenAPI 3.1.
pub trait JsonSchema {
    /// Generate a JSON Schema representation.
    fn json_schema(&self) -> Value;

    /// Whether this schema should be included in an object's `required` array.
    ///
    /// By default, fields are required. Schemas that accept a missing key
    /// return `false`: `.optional()`, `.nullable()` (a missing key parses as
    /// `None`), `.nullish()`, `.with_default(...)`, `.catch(...)`, and the
    /// null-accepting `vld::any()`, `vld::null()` and an unconstrained
    /// `vld::json_value()`.
    fn is_required(&self) -> bool {
        true
    }
}

/// Collects [`NestedSchemaEntry`] pairs from nested schemas.
///
/// Used by `schema!` / `#[derive(Validate)]` to discover named nested types
/// (`vld::nested!(T)`) that must be registered as OpenAPI components so the
/// emitted `$ref`s resolve.
pub trait CollectNestedSchemas {
    fn collect_nested_schemas(&self, _out: &mut Vec<NestedSchemaEntry>) {}
}

/// Hint for OpenAPI parameter location when a struct is used with `utoipa::IntoParams`.
///
/// Set via utoipa's `#[into_params(parameter_in = Query)]` attribute on a struct
/// defined with [`schema!`](crate::schema!) or `#[derive(Validate)]`.
pub trait OpenApiParameterIn {
    /// Returns `"query"`, `"path"`, `"header"`, or `"cookie"` when the struct is
    /// annotated with `#[into_params(parameter_in = ...)]`, otherwise `None`.
    fn parameter_in() -> Option<&'static str> {
        None
    }
}

/// Wrap a schema so it also accepts `null`: `{"anyOf": [inner, {"type": "null"}]}`.
///
/// Returns `inner` unchanged if it is already such a nullable `anyOf`.
pub fn nullable_schema(inner: Value) -> Value {
    let already_nullable = inner
        .get("anyOf")
        .and_then(Value::as_array)
        .is_some_and(|items| items.iter().any(|i| i.get("type") == Some(&json!("null"))));
    if already_nullable {
        return inner;
    }
    json!({ "anyOf": [inner, { "type": "null" }] })
}

// ---------------------------------------------------------------------------
// Impl helpers for wrapper schemas (used next to each type)
// ---------------------------------------------------------------------------

/// `JsonSchema` + `CollectNestedSchemas` forwarding to `inner_schema()`.
///
/// `impl[<generics>] Type, inner = S, where [extra bounds];`
macro_rules! forward_inner_schema {
    ($( impl[$($g:tt)*] $t:ty, inner = $S:ident $(, where [$($w:tt)*])? ; )*) => {$(
        impl<$($g)*> $crate::openapi::JsonSchema for $t
        where
            $S: $crate::schema::VldSchema + $crate::openapi::JsonSchema,
            $($($w)*)?
        {
            fn json_schema(&self) -> serde_json::Value {
                $crate::openapi::JsonSchema::json_schema(self.inner_schema())
            }
            fn is_required(&self) -> bool {
                $crate::openapi::JsonSchema::is_required(self.inner_schema())
            }
        }

        impl<$($g)*> $crate::openapi::CollectNestedSchemas for $t
        where
            $S: $crate::schema::VldSchema + $crate::openapi::CollectNestedSchemas,
            $($($w)*)?
        {
            fn collect_nested_schemas(&self, out: &mut Vec<$crate::schema::NestedSchemaEntry>) {
                $crate::openapi::CollectNestedSchemas::collect_nested_schemas(self.inner_schema(), out);
            }
        }
    )*};
}

pub(crate) use forward_inner_schema;

/// Collect-only forwarding for wrappers whose `JsonSchema` impl is custom.
macro_rules! collect_inner_schemas {
    ($( impl[$($g:tt)*] $t:ty, inner = $S:ident $(, where [$($w:tt)*])? ; )*) => {$(
        impl<$($g)*> $crate::openapi::CollectNestedSchemas for $t
        where
            $S: $crate::schema::VldSchema + $crate::openapi::CollectNestedSchemas,
            $($($w)*)?
        {
            fn collect_nested_schemas(&self, out: &mut Vec<$crate::schema::NestedSchemaEntry>) {
                $crate::openapi::CollectNestedSchemas::collect_nested_schemas(self.inner_schema(), out);
            }
        }
    )*};
}
pub(crate) use collect_inner_schemas;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Wrap a JSON Schema in a minimal OpenAPI 3.1 document structure.
///
/// The schema is placed under `#/components/schemas/{name}`. Nested `$ref`
/// targets are **not** added; types generated by `schema!` /
/// `#[derive(Validate)]` have a `to_openapi_document()` that includes them.
///
/// # Example
///
/// ```
/// use vld::openapi::{JsonSchema, to_openapi_document};
///
/// let schema = vld::string().email();
/// let doc = to_openapi_document("Email", &schema.json_schema());
/// assert_eq!(doc["openapi"], "3.1.0");
/// ```
pub fn to_openapi_document(name: &str, schema: &Value) -> Value {
    to_openapi_document_multi(&[(name, schema.clone())])
}

/// Wrap multiple schemas in an OpenAPI 3.1 document.
///
/// # Example
///
/// ```
/// use vld::openapi::{JsonSchema, to_openapi_document_multi};
///
/// let schemas = vec![
///     ("Name", vld::string().min(1).json_schema()),
///     ("Age", vld::number().int().min(0).json_schema()),
/// ];
/// let doc = to_openapi_document_multi(&schemas);
/// assert!(doc["components"]["schemas"]["Name"]["type"] == "string");
/// ```
pub fn to_openapi_document_multi(schemas: &[(&str, Value)]) -> Value {
    let map: serde_json::Map<String, Value> = schemas
        .iter()
        .map(|(name, schema)| (name.to_string(), schema.clone()))
        .collect();
    json!({
        "openapi": "3.1.0",
        "info": { "title": "API", "version": "1.0.0" },
        "paths": {},
        "components": { "schemas": Value::Object(map) }
    })
}
