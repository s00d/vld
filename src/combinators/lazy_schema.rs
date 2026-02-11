use serde_json::Value;
use std::marker::PhantomData;

use crate::error::VldError;
use crate::schema::VldSchema;

/// Lazy schema for recursive/self-referencing data structures.
///
/// The schema factory is called on each `parse_value` invocation,
/// which allows defining schemas that reference themselves.
///
/// Created via [`vld::lazy()`](crate::lazy).
///
/// # Example
/// ```ignore
/// // Recursive tree: { value: i64, children: Tree[] }
/// fn tree_schema() -> impl VldSchema<Output = serde_json::Value> {
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
    _phantom: PhantomData<T>,
}

impl<T, F> ZLazy<T, F>
where
    F: Fn() -> T,
    T: VldSchema,
{
    pub fn new(factory: F) -> Self {
        Self {
            factory,
            _phantom: PhantomData,
        }
    }
}

impl<T, F> VldSchema for ZLazy<T, F>
where
    F: Fn() -> T,
    T: VldSchema,
{
    type Output = T::Output;

    fn parse_value(&self, value: &Value) -> Result<T::Output, VldError> {
        let schema = (self.factory)();
        schema.parse_value(value)
    }
}
