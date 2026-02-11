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
/// // Parse string, then pipe to number validation
/// let schema = vld::string().coerce().pipe(vld::number().min(0.0));
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
    pub fn new(first: A, second: B) -> Self {
        Self { first, second }
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
            VldError::single(
                IssueCode::Custom {
                    code: "pipe_serialize".to_string(),
                },
                format!("Failed to serialize intermediate value in pipe: {}", e),
            )
        })?;
        self.second.parse_value(&json)
    }
}
