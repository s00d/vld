use serde_json::Value;

use crate::error::{IssueCode, VldError};
use crate::schema::VldSchema;

/// Object-safe trait for type-erased schema validation.
///
/// Used by [`ZObject`](crate::object::ZObject) and
/// [`ZDiscriminatedUnion`](crate::combinators::ZDiscriminatedUnion) to store
/// heterogeneous schemas. Every [`VldSchema`] with a `Serialize` output is a
/// `DynSchema`; with the `openapi` feature the schema must also implement
/// [`JsonSchema`](crate::openapi::JsonSchema) and
/// [`CollectNestedSchemas`](crate::openapi::CollectNestedSchemas) (as every
/// built-in schema does), so type-erased fields keep their JSON Schema.
pub trait DynSchema {
    /// Parse `value` and return the output serialized back to JSON.
    fn dyn_parse(&self, value: &Value) -> Result<Value, VldError>;

    /// [`VldSchema::parse_missing`] of the erased schema.
    fn dyn_parse_missing(&self) -> Result<Value, VldError>;

    /// JSON Schema of the erased schema.
    #[cfg(feature = "openapi")]
    fn dyn_json_schema(&self) -> Value;

    /// Whether this field belongs in an object's JSON Schema `required` array.
    #[cfg(feature = "openapi")]
    fn dyn_is_required(&self) -> bool;

    /// Collect named nested schemas (OpenAPI components) referenced by this schema.
    #[cfg(feature = "openapi")]
    fn dyn_collect_nested_schemas(&self, out: &mut Vec<crate::schema::NestedSchemaEntry>);
}

/// Serialize a validated output back to JSON (`SerializeError` on failure).
pub(crate) fn to_json_value<T: serde::Serialize + ?Sized>(out: &T) -> Result<Value, VldError> {
    serde_json::to_value(out).map_err(|e| {
        VldError::from_code(IssueCode::SerializeError {
            detail: e.to_string(),
        })
    })
}

#[cfg(not(feature = "openapi"))]
impl<T> DynSchema for T
where
    T: VldSchema,
    T::Output: serde::Serialize,
{
    fn dyn_parse(&self, value: &Value) -> Result<Value, VldError> {
        to_json_value(&self.parse_value(value)?)
    }

    fn dyn_parse_missing(&self) -> Result<Value, VldError> {
        to_json_value(&self.parse_missing()?)
    }
}

#[cfg(feature = "openapi")]
impl<T> DynSchema for T
where
    T: VldSchema + crate::openapi::JsonSchema + crate::openapi::CollectNestedSchemas,
    T::Output: serde::Serialize,
{
    fn dyn_parse(&self, value: &Value) -> Result<Value, VldError> {
        to_json_value(&self.parse_value(value)?)
    }

    fn dyn_parse_missing(&self) -> Result<Value, VldError> {
        to_json_value(&self.parse_missing()?)
    }

    fn dyn_json_schema(&self) -> Value {
        self.json_schema()
    }

    fn dyn_is_required(&self) -> bool {
        crate::openapi::JsonSchema::is_required(self)
    }

    fn dyn_collect_nested_schemas(&self, out: &mut Vec<crate::schema::NestedSchemaEntry>) {
        self.collect_nested_schemas(out);
    }
}
