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
    contains: Option<serde_json::Value>,
    min_contains: Option<usize>,
    max_contains: Option<usize>,
    unique: bool,
}

impl<T: VldSchema> ZArray<T> {
    pub fn new(element: T) -> Self {
        Self {
            element,
            min_len: None,
            max_len: None,
            exact_len: None,
            contains: None,
            min_contains: None,
            max_contains: None,
            unique: false,
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

    /// Require the array to contain this JSON value.
    pub fn contains(mut self, value: impl Into<serde_json::Value>) -> Self {
        self.contains = Some(value.into());
        self
    }

    /// Require at least this many matches of the `contains(...)` value.
    pub fn min_contains(mut self, n: usize) -> Self {
        self.min_contains = Some(n);
        self
    }

    /// Require at most this many matches of the `contains(...)` value.
    pub fn max_contains(mut self, n: usize) -> Self {
        self.max_contains = Some(n);
        self
    }

    /// Require all array items to be unique (by raw JSON equality).
    pub fn unique(mut self) -> Self {
        self.unique = true;
        self
    }

    #[allow(dead_code)]
    pub(crate) fn element_schema(&self) -> &T {
        &self.element
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
        if let Some(ref contains) = self.contains {
            schema["contains"] = contains.clone();
        }
        if let Some(min_contains) = self.min_contains {
            schema["minContains"] = serde_json::json!(min_contains);
        }
        if let Some(max_contains) = self.max_contains {
            schema["maxContains"] = serde_json::json!(max_contains);
        }
        if self.unique {
            schema["uniqueItems"] = serde_json::json!(true);
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

        if self.unique {
            for i in 0..arr.len() {
                for j in (i + 1)..arr.len() {
                    if arr[i] == arr[j] {
                        errors.push(
                            IssueCode::Custom {
                                code: "not_unique".to_string(),
                            },
                            "Array items must be unique",
                        );
                        break;
                    }
                }
            }
        }

        if let Some(ref contains) = self.contains {
            let count = arr.iter().filter(|v| *v == contains).count();
            if count == 0 {
                errors.push(
                    IssueCode::Custom {
                        code: "missing_contains".to_string(),
                    },
                    "Array must contain required value",
                );
            }
            if let Some(min_contains) = self.min_contains {
                if count < min_contains {
                    errors.push(
                        IssueCode::TooSmall {
                            minimum: min_contains as f64,
                            inclusive: true,
                        },
                        format!(
                            "Array must contain required value at least {} time(s)",
                            min_contains
                        ),
                    );
                }
            }
            if let Some(max_contains) = self.max_contains {
                if count > max_contains {
                    errors.push(
                        IssueCode::TooBig {
                            maximum: max_contains as f64,
                            inclusive: true,
                        },
                        format!(
                            "Array must contain required value at most {} time(s)",
                            max_contains
                        ),
                    );
                }
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
