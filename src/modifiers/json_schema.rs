//! [`JsonSchema`] / [`CollectNestedSchemas`] impls for the modifiers.

use serde_json::Value;

use crate::openapi::{nullable_schema, JsonSchema};
use crate::schema::VldSchema;

crate::openapi::collect_inner_schemas! {
    impl[S] crate::modifiers::ZOptional<S>, inner = S;
    impl[S] crate::modifiers::ZNullable<S>, inner = S;
    impl[S] crate::modifiers::ZNullish<S>, inner = S;
    impl[S] crate::modifiers::ZDefault<S>, inner = S, where [S::Output: Clone];
}

impl<S: VldSchema + JsonSchema> JsonSchema for crate::modifiers::ZOptional<S> {
    fn json_schema(&self) -> Value {
        nullable_schema(self.inner_schema().json_schema())
    }
    fn is_required(&self) -> bool {
        false
    }
}

/// Like `optional()`, a missing key parses as `None`, so it is not `required`.
impl<S: VldSchema + JsonSchema> JsonSchema for crate::modifiers::ZNullable<S> {
    fn json_schema(&self) -> Value {
        nullable_schema(self.inner_schema().json_schema())
    }
    fn is_required(&self) -> bool {
        false
    }
}

impl<S: VldSchema + JsonSchema> JsonSchema for crate::modifiers::ZNullish<S> {
    fn json_schema(&self) -> Value {
        nullable_schema(self.inner_schema().json_schema())
    }
    fn is_required(&self) -> bool {
        false
    }
}

impl<S: VldSchema + JsonSchema> JsonSchema for crate::modifiers::ZDefault<S>
where
    S::Output: Clone + serde::Serialize,
{
    fn json_schema(&self) -> Value {
        let mut schema = self.inner_schema().json_schema();
        if let (Some(obj), Ok(default)) = (
            schema.as_object_mut(),
            serde_json::to_value(self.default_value()),
        ) {
            obj.insert("default".to_string(), default);
        }
        schema
    }
    fn is_required(&self) -> bool {
        false
    }
}
