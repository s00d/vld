use serde_json::Value;
use std::marker::PhantomData;

use crate::error::VldError;
use crate::schema::VldSchema;

/// A `(component_name, json_schema_fn)` pair describing a named nested schema.
///
/// Re-exported as `vld::openapi::NestedSchemaEntry`.
pub type NestedSchemaEntry = (&'static str, fn() -> serde_json::Value);

/// Collector for transitive nested OpenAPI component schemas.
///
/// Shared by `NestedSchema` and the `nested!` macro.
pub type NestedSchemaCollectFn = fn(&mut Vec<NestedSchemaEntry>);

/// Schema for parsing nested structures. Created via [`vld::nested()`](crate::nested())
/// or the [`vld::nested!`](crate::nested!) macro.
pub struct NestedSchema<T, F>
where
    F: Fn(&Value) -> Result<T, VldError>,
{
    parse_fn: F,
    #[cfg_attr(not(feature = "openapi"), allow(dead_code))]
    pub(crate) name: Option<&'static str>,
    /// Returns the full JSON Schema of the nested type (for OpenAPI component registration).
    #[cfg_attr(not(feature = "openapi"), allow(dead_code))]
    pub(crate) json_schema_fn: Option<fn() -> serde_json::Value>,
    /// Collects transitive nested schemas of the nested type into a shared buffer.
    #[cfg_attr(not(feature = "openapi"), allow(dead_code))]
    pub(crate) collect_nested_fn: Option<NestedSchemaCollectFn>,
    _phantom: PhantomData<T>,
}

impl<T, F> NestedSchema<T, F>
where
    F: Fn(&Value) -> Result<T, VldError>,
{
    pub fn new(f: F) -> Self {
        Self {
            parse_fn: f,
            name: None,
            json_schema_fn: None,
            collect_nested_fn: None,
            _phantom: PhantomData,
        }
    }

    pub fn new_named(
        f: F,
        name: &'static str,
        json_schema_fn: Option<fn() -> serde_json::Value>,
        collect_nested_fn: Option<NestedSchemaCollectFn>,
    ) -> Self {
        Self {
            parse_fn: f,
            name: Some(name),
            json_schema_fn,
            collect_nested_fn,
            _phantom: PhantomData,
        }
    }
}

impl<T, F> VldSchema for NestedSchema<T, F>
where
    F: Fn(&Value) -> Result<T, VldError>,
{
    type Output = T;
    fn parse_value(&self, value: &Value) -> Result<T, VldError> {
        (self.parse_fn)(value)
    }
}

#[cfg(feature = "openapi")]
impl<T, F> crate::openapi::JsonSchema for NestedSchema<T, F>
where
    F: Fn(&Value) -> Result<T, VldError>,
{
    fn json_schema(&self) -> Value {
        match self.name {
            Some(name) => serde_json::json!({ "$ref": format!("#/components/schemas/{}", name) }),
            // Unnamed nested schemas are opaque closures.
            None => serde_json::json!({ "type": "object" }),
        }
    }
}

#[cfg(feature = "openapi")]
impl<T, F> crate::openapi::CollectNestedSchemas for NestedSchema<T, F>
where
    F: Fn(&Value) -> Result<T, VldError>,
{
    fn collect_nested_schemas(&self, out: &mut Vec<NestedSchemaEntry>) {
        if let (Some(name), Some(f)) = (self.name, self.json_schema_fn) {
            // Dedup by name: skip cycles and diamond references already visited.
            if out.iter().any(|(n, _)| *n == name) {
                return;
            }
            out.push((name, f));
            if let Some(collect) = self.collect_nested_fn {
                collect(out);
            }
        }
    }
}
