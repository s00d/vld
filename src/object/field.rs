//! Type-erased field wrappers used by [`ZObject`](super::ZObject).

use serde_json::Value;

use crate::error::{IssueCode, VldError};
use crate::schema::DynSchema;
#[cfg(feature = "openapi")]
use crate::schema::VldSchema;

#[cfg(feature = "openapi")]
/// Wrapper that stores a schema implementing both `DynSchema` and `JsonSchema`.
///
/// Used by [`ZObject::field_schema()`] to include JSON Schema info for fields.
pub(crate) struct JsonSchemaField<T> {
    pub(crate) inner: T,
}

#[cfg(feature = "openapi")]
impl<T> DynSchema for JsonSchemaField<T>
where
    T: VldSchema + crate::openapi::JsonSchema + crate::openapi::CollectNestedSchemas,
    T::Output: serde::Serialize,
{
    fn dyn_parse(&self, value: &Value) -> Result<Value, VldError> {
        DynSchema::dyn_parse(&self.inner, value)
    }

    fn dyn_json_schema(&self) -> Value {
        self.inner.json_schema()
    }

    fn dyn_is_required(&self) -> bool {
        crate::openapi::JsonSchema::is_required(&self.inner)
    }

    fn dyn_collect_nested_schemas(&self, out: &mut Vec<crate::schema::NestedSchemaEntry>) {
        self.inner.collect_nested_schemas(out);
    }
}

/// Internal wrapper that makes a DynSchema nullable (null/missing → Value::Null).
///
/// On null/missing the inner schema still runs first so defaults and
/// `.catch()` fallbacks apply; only if it rejects null is `Value::Null` returned.
pub(crate) struct OptionalDynSchema(pub(crate) Box<dyn DynSchema>);

impl DynSchema for OptionalDynSchema {
    fn dyn_parse(&self, value: &Value) -> Result<Value, VldError> {
        if value.is_null() {
            return Ok(self.0.dyn_parse(value).unwrap_or(Value::Null));
        }
        self.0.dyn_parse(value)
    }

    #[cfg(feature = "openapi")]
    fn dyn_json_schema(&self) -> Value {
        crate::openapi::nullable_schema(self.0.dyn_json_schema())
    }

    #[cfg(feature = "openapi")]
    fn dyn_is_required(&self) -> bool {
        false
    }

    #[cfg(feature = "openapi")]
    fn dyn_collect_nested_schemas(&self, out: &mut Vec<crate::schema::NestedSchemaEntry>) {
        self.0.dyn_collect_nested_schemas(out);
    }
}

/// Internal wrapper that rejects null values.
pub(crate) struct RequiredDynSchema(pub(crate) Box<dyn DynSchema>);

impl DynSchema for RequiredDynSchema {
    fn dyn_parse(&self, value: &Value) -> Result<Value, VldError> {
        if value.is_null() {
            return Err(VldError::single(
                IssueCode::MissingField,
                "Required field is missing or null",
            ));
        }
        self.0.dyn_parse(value)
    }

    #[cfg(feature = "openapi")]
    fn dyn_json_schema(&self) -> Value {
        self.0.dyn_json_schema()
    }

    #[cfg(feature = "openapi")]
    fn dyn_is_required(&self) -> bool {
        true
    }

    #[cfg(feature = "openapi")]
    fn dyn_collect_nested_schemas(&self, out: &mut Vec<crate::schema::NestedSchemaEntry>) {
        self.0.dyn_collect_nested_schemas(out);
    }
}
