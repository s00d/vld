use serde_json::Value;
use std::collections::HashSet;

use crate::error::{value_type_name, IssueCode, PathSegment, SizeOrigin, VldError};
use crate::schema::VldSchema;

/// Schema for validating a JSON array into a `HashSet` (unique elements).
///
/// Created via [`vld::set()`](crate::set).
///
/// Duplicates (after validation) are silently merged, so the generated JSON
/// Schema does **not** declare `uniqueItems` (duplicate input is valid).
///
/// # Example
/// ```ignore
/// let schema = vld::set(vld::string().min(1));
/// // Input: ["a", "b", "a"]
/// // Output: HashSet { "a", "b" }
/// ```
pub struct ZSet<T: VldSchema> {
    element: T,
    min_size: Option<usize>,
    max_size: Option<usize>,
}

impl<T: VldSchema> ZSet<T> {
    pub fn new(element: T) -> Self {
        Self {
            element,
            min_size: None,
            max_size: None,
        }
    }

    /// Minimum number of unique elements.
    pub fn min_size(mut self, n: usize) -> Self {
        self.min_size = Some(n);
        self
    }

    /// Maximum number of unique elements.
    pub fn max_size(mut self, n: usize) -> Self {
        self.max_size = Some(n);
        self
    }

    #[allow(dead_code)]
    pub(crate) fn element_schema(&self) -> &T {
        &self.element
    }

    /// Generate a JSON Schema (called by [`JsonSchema`](crate::openapi::JsonSchema) trait impl).
    ///
    /// `min_size` becomes `minItems` (n unique items need at least n items);
    /// `max_size` is *not* emitted as `maxItems`, because duplicates in the
    /// input are allowed and do not count towards the limit. The bound is
    /// exposed as `x-maxUniqueItems` instead.
    ///
    /// Requires the `openapi` feature.
    #[cfg(feature = "openapi")]
    pub fn to_json_schema_inner(&self) -> serde_json::Value
    where
        T: crate::openapi::JsonSchema,
    {
        let mut schema = serde_json::json!({
            "type": "array",
            "items": self.element.json_schema(),
        });
        if let Some(min) = self.min_size {
            schema["minItems"] = serde_json::json!(min);
        }
        if let Some(max) = self.max_size {
            schema["x-maxUniqueItems"] = serde_json::json!(max);
        }
        schema
    }
}

impl<T> VldSchema for ZSet<T>
where
    T: VldSchema,
    T::Output: Eq + std::hash::Hash,
{
    type Output = HashSet<T::Output>;

    fn parse_value(&self, value: &Value) -> Result<Self::Output, VldError> {
        let arr = value.as_array().ok_or_else(|| {
            VldError::from_code(IssueCode::InvalidType {
                expected: "array".to_string(),
                received: value_type_name(value),
            })
        })?;

        let mut result = HashSet::new();
        let mut errors = VldError::new();

        for (i, item) in arr.iter().enumerate() {
            match self.element.parse_value(item) {
                Ok(v) => {
                    result.insert(v);
                }
                Err(e) => {
                    errors = errors.merge(e.with_prefix(PathSegment::Index(i)));
                }
            }
        }

        if let Some(min) = self.min_size {
            if result.len() < min {
                errors.push_code(IssueCode::TooSmall {
                    origin: SizeOrigin::Set,
                    minimum: min.into(),
                    inclusive: true,
                });
            }
        }

        if let Some(max) = self.max_size {
            if result.len() > max {
                errors.push_code(IssueCode::TooBig {
                    origin: SizeOrigin::Set,
                    maximum: max.into(),
                    inclusive: true,
                });
            }
        }

        if errors.is_empty() {
            Ok(result)
        } else {
            Err(errors)
        }
    }
}
