use serde_json::Value;

use crate::error::{IssueCode, VldError};
use crate::schema::VldSchema;

/// Chains two schemas: output of the first is serialized to JSON, then parsed by the second.
///
/// Created via [`VldSchema::pipe()`].
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// // Trim a string, then validate it as a non-negative number.
/// // The second schema receives a JSON *string*, so it needs `.coerce()`.
/// let schema = vld::string().trim().pipe(vld::number().coerce().min(0.0));
/// assert_eq!(schema.parse(r#"" 5 ""#).unwrap(), 5.0);
/// assert!(schema.parse(r#""-1""#).is_err());
/// ```
pub struct ZPipe<A, B>
where
    A: VldSchema,
    B: VldSchema,
    A::Output: serde::Serialize,
{
    first: A,
    second: B,
}

impl<A, B> ZPipe<A, B>
where
    A: VldSchema,
    B: VldSchema,
    A::Output: serde::Serialize,
{
    /// Create the schema (the same as its `vld::` constructor function).
    pub fn new(first: A, second: B) -> Self {
        Self { first, second }
    }

    /// Access the first (input) stage.
    pub fn first_schema(&self) -> &A {
        &self.first
    }

    /// Access the second (output) stage.
    pub fn second_schema(&self) -> &B {
        &self.second
    }
}

impl<A, B> VldSchema for ZPipe<A, B>
where
    A: VldSchema,
    B: VldSchema,
    A::Output: serde::Serialize,
{
    type Output = B::Output;

    fn parse_value(&self, value: &Value) -> Result<B::Output, VldError> {
        let intermediate = self.first.parse_value(value)?;
        let json = serde_json::to_value(&intermediate).map_err(|e| {
            VldError::from_code(IssueCode::SerializeError {
                detail: e.to_string(),
            })
        })?;
        self.second.parse_value(&json)
    }
}
