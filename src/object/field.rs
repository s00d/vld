//! Type-erased field wrappers used by [`ZObject`](super::ZObject).

use serde_json::Value;

use crate::error::{IssueCode, VldError};
use crate::schema::DynSchema;

/// Internal wrapper that makes a DynSchema nullable (null/missing → Value::Null).
///
/// On null/missing the inner schema still runs first so defaults and
/// `.catch()` fallbacks apply; only if it rejects null is `Value::Null` returned.
pub(crate) struct OptionalDynSchema(pub(crate) Box<dyn DynSchema + Send + Sync>);

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
pub(crate) struct RequiredDynSchema(pub(crate) Box<dyn DynSchema + Send + Sync>);

impl DynSchema for RequiredDynSchema {
    fn dyn_parse(&self, value: &Value) -> Result<Value, VldError> {
        if value.is_null() {
            return Err(VldError::from_code(IssueCode::MissingField));
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
