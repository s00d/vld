//! JSON Schema / OpenAPI generation from `vld` schemas.
//!
//! The [`JsonSchema`] trait is implemented by schema types that can produce a
//! [JSON Schema](https://json-schema.org/) representation. The output is
//! compatible with **OpenAPI 3.1** (which uses JSON Schema directly).
//!
//! # Example
//!
//! ```
//! use vld::prelude::*;
//! use vld::json_schema::JsonSchema;
//!
//! let schema = vld::string().min(2).max(50).email();
//! let js = schema.json_schema();
//! assert_eq!(js["type"], "string");
//! assert_eq!(js["minLength"], 2);
//! assert_eq!(js["format"], "email");
//! ```

use serde_json::Value;

/// Trait for types that can produce a JSON Schema representation.
///
/// Implemented for all core `vld` schemas. The output is a `serde_json::Value`
/// representing the JSON Schema object, compatible with OpenAPI 3.1.
pub trait JsonSchema {
    /// Generate a JSON Schema representation.
    fn json_schema(&self) -> Value;
}

// ---------------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------------

impl JsonSchema for crate::primitives::ZString {
    fn json_schema(&self) -> Value {
        self.to_json_schema()
    }
}

impl JsonSchema for crate::primitives::ZNumber {
    fn json_schema(&self) -> Value {
        self.to_json_schema()
    }
}

impl JsonSchema for crate::primitives::ZInt {
    fn json_schema(&self) -> Value {
        self.to_json_schema()
    }
}

impl JsonSchema for crate::primitives::ZBoolean {
    fn json_schema(&self) -> Value {
        self.to_json_schema()
    }
}

impl JsonSchema for crate::primitives::ZEnum {
    fn json_schema(&self) -> Value {
        self.to_json_schema()
    }
}

impl JsonSchema for crate::primitives::ZAny {
    fn json_schema(&self) -> Value {
        self.to_json_schema()
    }
}

// ---------------------------------------------------------------------------
// Date/DateTime (chrono feature)
// ---------------------------------------------------------------------------

#[cfg(feature = "chrono")]
impl JsonSchema for crate::primitives::ZDate {
    fn json_schema(&self) -> Value {
        self.to_json_schema()
    }
}

#[cfg(feature = "chrono")]
impl JsonSchema for crate::primitives::ZDateTime {
    fn json_schema(&self) -> Value {
        self.to_json_schema()
    }
}

// ---------------------------------------------------------------------------
// Collections
// ---------------------------------------------------------------------------

impl<T: crate::schema::VldSchema + JsonSchema> JsonSchema for crate::collections::ZArray<T> {
    fn json_schema(&self) -> Value {
        self.to_json_schema_inner()
    }
}

impl<V: crate::schema::VldSchema + JsonSchema> JsonSchema for crate::collections::ZRecord<V> {
    fn json_schema(&self) -> Value {
        self.to_json_schema_inner()
    }
}

impl<T> JsonSchema for crate::collections::ZSet<T>
where
    T: crate::schema::VldSchema + JsonSchema,
    T::Output: Eq + std::hash::Hash,
{
    fn json_schema(&self) -> Value {
        self.to_json_schema_inner()
    }
}

// ---------------------------------------------------------------------------
// Modifiers
// ---------------------------------------------------------------------------

impl<S: crate::schema::VldSchema + JsonSchema> JsonSchema for crate::modifiers::ZOptional<S> {
    fn json_schema(&self) -> Value {
        // oneOf: [inner, {type: "null"}] — represents optional/nullable in OpenAPI 3.1
        let inner = self.inner_schema().json_schema();
        serde_json::json!({
            "oneOf": [inner, {"type": "null"}]
        })
    }
}

impl<S: crate::schema::VldSchema + JsonSchema> JsonSchema for crate::modifiers::ZNullable<S> {
    fn json_schema(&self) -> Value {
        let inner = self.inner_schema().json_schema();
        serde_json::json!({
            "oneOf": [inner, {"type": "null"}]
        })
    }
}

impl<S: crate::schema::VldSchema + JsonSchema> JsonSchema for crate::modifiers::ZNullish<S> {
    fn json_schema(&self) -> Value {
        let inner = self.inner_schema().json_schema();
        serde_json::json!({
            "oneOf": [inner, {"type": "null"}]
        })
    }
}

impl<S: crate::schema::VldSchema + JsonSchema> JsonSchema for crate::modifiers::ZDefault<S>
where
    S::Output: Clone,
{
    fn json_schema(&self) -> Value {
        self.inner_schema().json_schema()
    }
}

impl<S: crate::schema::VldSchema + JsonSchema> JsonSchema for crate::combinators::ZCatch<S>
where
    S::Output: Clone,
{
    fn json_schema(&self) -> Value {
        self.inner_schema().json_schema()
    }
}

// ---------------------------------------------------------------------------
// Combinators
// ---------------------------------------------------------------------------

impl<S: crate::schema::VldSchema + JsonSchema, F> JsonSchema for crate::combinators::ZRefine<S, F>
where
    F: Fn(&S::Output) -> bool,
{
    fn json_schema(&self) -> Value {
        self.inner_schema().json_schema()
    }
}

impl<S: crate::schema::VldSchema + JsonSchema, F, U> JsonSchema
    for crate::combinators::ZTransform<S, F, U>
where
    F: Fn(S::Output) -> U,
{
    fn json_schema(&self) -> Value {
        // Transform doesn't change the input schema
        self.inner_schema().json_schema()
    }
}

impl<S: crate::schema::VldSchema + JsonSchema> JsonSchema for crate::combinators::ZDescribe<S> {
    fn json_schema(&self) -> Value {
        let mut schema = self.inner_schema().json_schema();
        let desc = self.description();
        if !desc.is_empty() {
            schema["description"] = Value::String(desc.to_string());
        }
        schema
    }
}

impl<A, B> JsonSchema for crate::combinators::ZUnion2<A, B>
where
    A: crate::schema::VldSchema + JsonSchema,
    B: crate::schema::VldSchema + JsonSchema,
{
    fn json_schema(&self) -> Value {
        serde_json::json!({
            "oneOf": [self.schema_a().json_schema(), self.schema_b().json_schema()]
        })
    }
}

impl<A, B, C> JsonSchema for crate::combinators::ZUnion3<A, B, C>
where
    A: crate::schema::VldSchema + JsonSchema,
    B: crate::schema::VldSchema + JsonSchema,
    C: crate::schema::VldSchema + JsonSchema,
{
    fn json_schema(&self) -> Value {
        serde_json::json!({
            "oneOf": [
                self.schema_a().json_schema(),
                self.schema_b().json_schema(),
                self.schema_c().json_schema(),
            ]
        })
    }
}

impl<A, B> JsonSchema for crate::combinators::ZIntersection<A, B>
where
    A: crate::schema::VldSchema + JsonSchema,
    B: crate::schema::VldSchema + JsonSchema,
{
    fn json_schema(&self) -> Value {
        serde_json::json!({
            "allOf": [self.schema_a().json_schema(), self.schema_b().json_schema()]
        })
    }
}

// ---------------------------------------------------------------------------
// NestedSchema — generic fallback (opaque nested schema)
// ---------------------------------------------------------------------------

impl<T, F> JsonSchema for crate::schema::NestedSchema<T, F>
where
    F: Fn(&serde_json::Value) -> Result<T, crate::error::VldError>,
{
    fn json_schema(&self) -> Value {
        // Nested schemas are opaque closures; emit a generic object schema.
        serde_json::json!({"type": "object"})
    }
}

// ---------------------------------------------------------------------------
// Object (dynamic)
// ---------------------------------------------------------------------------

impl JsonSchema for crate::object::ZObject {
    fn json_schema(&self) -> Value {
        self.to_json_schema()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Wrap a JSON Schema in a minimal OpenAPI 3.1 document structure.
///
/// The schema is placed under `#/components/schemas/{name}`.
///
/// # Example
///
/// ```
/// use vld::json_schema::{JsonSchema, to_openapi_document};
///
/// let schema = vld::string().email();
/// let doc = to_openapi_document("Email", &schema.json_schema());
/// assert_eq!(doc["openapi"], "3.1.0");
/// ```
pub fn to_openapi_document(name: &str, schema: &Value) -> Value {
    serde_json::json!({
        "openapi": "3.1.0",
        "info": { "title": "API", "version": "1.0.0" },
        "paths": {},
        "components": {
            "schemas": {
                name: schema
            }
        }
    })
}

/// Wrap multiple schemas in an OpenAPI 3.1 document.
///
/// # Example
///
/// ```
/// use vld::json_schema::{JsonSchema, to_openapi_document_multi};
///
/// let schemas = vec![
///     ("Name", vld::string().min(1).json_schema()),
///     ("Age", vld::number().int().min(0).json_schema()),
/// ];
/// let doc = to_openapi_document_multi(&schemas);
/// assert!(doc["components"]["schemas"]["Name"]["type"] == "string");
/// ```
pub fn to_openapi_document_multi(schemas: &[(&str, Value)]) -> Value {
    let mut map = serde_json::Map::new();
    for (name, schema) in schemas {
        map.insert(name.to_string(), schema.clone());
    }
    serde_json::json!({
        "openapi": "3.1.0",
        "info": { "title": "API", "version": "1.0.0" },
        "paths": {},
        "components": {
            "schemas": Value::Object(map)
        }
    })
}
