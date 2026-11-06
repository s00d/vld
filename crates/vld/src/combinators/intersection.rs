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
    /// Create the schema (the same as its `vld::` constructor function).
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
        // An inner `Err` may carry no issues: track failure, not issue count.
        let mut failed = false;

        let first_result = match self.first.parse_value(value) {
            Ok(v) => Some(v),
            Err(e) => {
                failed = true;
                errors = errors.merge(e);
                None
            }
        };

        if let Err(e) = self.second.parse_value(value) {
            failed = true;
            errors = errors.merge(e);
        }

        match first_result {
            Some(v) if !failed => Ok(v),
            _ => Err(errors),
        }
    }
}
