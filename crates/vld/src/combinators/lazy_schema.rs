use serde_json::Value;
use std::sync::OnceLock;

use crate::error::VldError;
use crate::schema::VldSchema;

/// Lazy schema for recursive/self-referencing data structures.
///
/// The schema factory is called on the first `parse_value` of this node and the
/// result is cached, which allows defining schemas that reference themselves
/// without rebuilding them on every parse.
///
/// Created via [`vld::lazy()`](crate::lazy).
///
/// Every `vld::lazy(...)` node calls its factory once; nested lazy nodes
/// built by the factory are themselves created on first use, so a recursive
/// schema unfolds only as deep as the inputs go.
///
/// # Example
/// ```ignore
/// // Recursive tree: { value: i64, children: Tree[] }
/// fn tree_schema() -> impl VldSchema<Output = serde_json::Map<String, serde_json::Value>> {
///     vld::object()
///         .field("value", vld::number().int())
///         .field("children", vld::array(vld::lazy(tree_schema)))
/// }
/// ```
pub struct ZLazy<T, F>
where
    F: Fn() -> T,
    T: VldSchema,
{
    factory: F,
    schema: OnceLock<T>,
}

impl<T, F> ZLazy<T, F>
where
    F: Fn() -> T,
    T: VldSchema,
{
    /// Create the schema (the same as its `vld::` constructor function).
    pub fn new(factory: F) -> Self {
        Self {
            factory,
            schema: OnceLock::new(),
        }
    }

    /// The (cached) schema produced by the factory.
    pub fn schema(&self) -> &T {
        self.schema.get_or_init(|| (self.factory)())
    }
}

impl<T, F> VldSchema for ZLazy<T, F>
where
    F: Fn() -> T,
    T: VldSchema,
{
    type Output = T::Output;

    fn parse_value(&self, value: &Value) -> Result<T::Output, VldError> {
        self.schema().parse_value(value)
    }
}
