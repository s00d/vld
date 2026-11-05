use serde_json::Value;

use crate::error::{IssueCode, VldError};
use crate::schema::VldSchema;

/// Object-safe trait for type-erased schema validation.
///
/// Used internally by [`ZObject`] to store heterogeneous field schemas.
pub trait DynSchema {
    fn dyn_parse(&self, value: &Value) -> Result<Value, VldError>;
    /// Generate a JSON Schema for this field. Returns empty schema `{}` by default.
    ///
    /// Only available with the `openapi` feature.
    #[cfg(feature = "openapi")]
    fn dyn_json_schema(&self) -> Value {
        serde_json::json!({})
    }

    /// Whether this field should appear in an object's JSON Schema `required` array.
    ///
    /// Only available with the `openapi` feature.
    #[cfg(feature = "openapi")]
    fn dyn_is_required(&self) -> bool {
        true
    }

    /// Collect named nested schemas (OpenAPI components) referenced by this field.
    ///
    /// Only available with the `openapi` feature.
    #[cfg(feature = "openapi")]
    fn dyn_collect_nested_schemas(&self, _out: &mut Vec<crate::schema::NestedSchemaEntry>) {}
}

/// Blanket implementation: any `VldSchema` whose output is `Serialize`
/// can be used as a dynamic schema.
impl<T> DynSchema for T
where
    T: VldSchema,
    T::Output: serde::Serialize,
{
    fn dyn_parse(&self, value: &Value) -> Result<Value, VldError> {
        let result = self.parse_value(value)?;
        serde_json::to_value(&result).map_err(|e| {
            VldError::single(
                IssueCode::Custom {
                    code: "serialize".to_string(),
                },
                format!("Failed to serialize validated value: {}", e),
            )
        })
    }
}
