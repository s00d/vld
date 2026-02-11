use serde_json::Value;

use crate::error::{value_type_name, IssueCode, PathSegment, VldError};
use crate::schema::VldSchema;

/// Schema for array validation. Created via [`vld::array()`](crate::array).
///
/// Validates each element using the provided element schema.
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// let schema = vld::array(vld::string().min(1)).min_len(1).max_len(10);
/// ```
pub struct ZArray<T: VldSchema> {
    element: T,
    min_len: Option<usize>,
    max_len: Option<usize>,
    exact_len: Option<usize>,
}

impl<T: VldSchema> ZArray<T> {
    pub fn new(element: T) -> Self {
        Self {
            element,
            min_len: None,
            max_len: None,
            exact_len: None,
        }
    }

    /// Minimum number of elements.
    pub fn min_len(mut self, len: usize) -> Self {
        self.min_len = Some(len);
        self
    }

    /// Maximum number of elements.
    pub fn max_len(mut self, len: usize) -> Self {
        self.max_len = Some(len);
        self
    }

    /// Exact number of elements.
    pub fn len(mut self, len: usize) -> Self {
        self.exact_len = Some(len);
        self
    }

    /// Alias for `min_len(1)` — array must not be empty.
    pub fn non_empty(self) -> Self {
        self.min_len(1)
    }

    /// Generate a JSON Schema (called by [`JsonSchema`](crate::json_schema::JsonSchema) trait impl).
    ///
    /// Requires the `openapi` feature.
    #[cfg(feature = "openapi")]
    pub fn to_json_schema_inner(&self) -> serde_json::Value
    where
        T: crate::json_schema::JsonSchema,
    {
        let mut schema = serde_json::json!({
            "type": "array",
            "items": self.element.json_schema(),
        });
        if let Some(min) = self.min_len {
            schema["minItems"] = serde_json::json!(min);
        }
        if let Some(max) = self.max_len {
            schema["maxItems"] = serde_json::json!(max);
        }
        if let Some(exact) = self.exact_len {
            schema["minItems"] = serde_json::json!(exact);
            schema["maxItems"] = serde_json::json!(exact);
        }
        schema
    }
}

impl<T: VldSchema> VldSchema for ZArray<T> {
    type Output = Vec<T::Output>;

    fn parse_value(&self, value: &Value) -> Result<Vec<T::Output>, VldError> {
        let arr = value.as_array().ok_or_else(|| {
            VldError::single(
                IssueCode::InvalidType {
                    expected: "array".to_string(),
                    received: value_type_name(value),
                },
                format!("Expected array, received {}", value_type_name(value)),
            )
        })?;

        let mut errors = VldError::new();

        // Length checks
        if let Some(min) = self.min_len {
            if arr.len() < min {
                errors.push(
                    IssueCode::TooSmall {
                        minimum: min as f64,
                        inclusive: true,
                    },
                    format!("Array must have at least {} elements", min),
                );
            }
        }

        if let Some(max) = self.max_len {
            if arr.len() > max {
                errors.push(
                    IssueCode::TooBig {
                        maximum: max as f64,
                        inclusive: true,
                    },
                    format!("Array must have at most {} elements", max),
                );
            }
        }

        if let Some(exact) = self.exact_len {
            if arr.len() != exact {
                errors.push(
                    IssueCode::Custom {
                        code: "invalid_length".to_string(),
                    },
                    format!("Array must have exactly {} elements", exact),
                );
            }
        }

        // Validate each element
        let mut results = Vec::with_capacity(arr.len());

        for (i, item) in arr.iter().enumerate() {
            match self.element.parse_value(item) {
                Ok(v) => results.push(v),
                Err(e) => {
                    errors = errors.merge(e.with_prefix(PathSegment::Index(i)));
                }
            }
        }

        if errors.is_empty() {
            Ok(results)
        } else {
            Err(errors)
        }
    }
}
