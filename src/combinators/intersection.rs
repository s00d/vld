use serde_json::Value;

use crate::error::VldError;
use crate::schema::VldSchema;

/// Intersection of two schemas: input must satisfy both.
///
/// Both schemas are run on the same input. If both succeed, the output of the
/// **first** schema is returned. If either fails, all errors are merged.
///
/// Created via [`vld::intersection()`](crate::intersection).
///
/// # Example
/// ```ignore
/// let schema = vld::intersection(
///     vld::string().min(3),
///     vld::string().email(),
/// );
/// ```
pub struct ZIntersection<A: VldSchema, B: VldSchema> {
    first: A,
    second: B,
}

impl<A: VldSchema, B: VldSchema> ZIntersection<A, B> {
    pub fn new(first: A, second: B) -> Self {
        Self { first, second }
    }

    /// Access the first schema.
    pub fn schema_a(&self) -> &A {
        &self.first
    }
    /// Access the second schema.
    pub fn schema_b(&self) -> &B {
        &self.second
    }
}

impl<A: VldSchema, B: VldSchema> VldSchema for ZIntersection<A, B> {
    type Output = A::Output;

    fn parse_value(&self, value: &Value) -> Result<A::Output, VldError> {
        let mut errors = VldError::new();

        let first_result = match self.first.parse_value(value) {
            Ok(v) => Some(v),
            Err(e) => {
                errors = errors.merge(e);
                None
            }
        };

        if let Err(e) = self.second.parse_value(value) {
            errors = errors.merge(e);
        }

        if errors.is_empty() {
            Ok(first_result.unwrap())
        } else {
            Err(errors)
        }
    }
}
