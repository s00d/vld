use serde_json::Value;

use crate::error::{IssueCode, PathSegment, SizeOrigin, VldError};
use crate::schema::VldSchema;

/// Canonical text form for duplicate detection: object keys sorted, integral
/// numbers written as integers (so `1` and `1.0` collide).
fn canonical(v: &Value) -> String {
    match v {
        Value::Number(n) => match (n.as_i64(), n.as_u64(), n.as_f64()) {
            (Some(i), ..) => i.to_string(),
            (_, Some(u), _) => u.to_string(),
            (_, _, Some(f)) if f.fract() == 0.0 && f.abs() < 9.2e18 => (f as i64).to_string(),
            _ => n.to_string(),
        },
        Value::Array(a) => {
            let items: Vec<String> = a.iter().map(canonical).collect();
            format!("[{}]", items.join(","))
        }
        Value::Object(o) => {
            let mut items: Vec<String> = o
                .iter()
                .map(|(k, v)| format!("{}:{}", Value::String(k.clone()), canonical(v)))
                .collect();
            items.sort();
            format!("{{{}}}", items.join(","))
        }
        other => other.to_string(),
    }
}

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

    /// Require the array to contain this JSON value (numbers compare by value,
    /// so `1` matches `1.0`). Emitted in JSON Schema as `"contains": {"const": value}`.
    pub fn contains(mut self, value: impl Into<serde_json::Value>) -> Self {
        self.contains = Some(value.into());
        self
    }

    /// Require at least this many matches of the `contains(...)` value
    /// (default `1`; `0` makes the value optional). Has no effect without `contains`.
    pub fn min_contains(mut self, n: usize) -> Self {
        self.min_contains = Some(n);
        self
    }

    /// Require at most this many matches of the `contains(...)` value.
    /// Has no effect without `contains`.
    pub fn max_contains(mut self, n: usize) -> Self {
        self.max_contains = Some(n);
        self
    }

    /// Require all array items to be unique (JSON equality, numbers compared by
    /// value: `1` and `1.0` are duplicates). Each repeated item is reported once,
    /// at its own index.
    pub fn unique(mut self) -> Self {
        self.unique = true;
        self
    }
}

impl<T: VldSchema> VldSchema for ZArray<T> {
    type Output = Vec<T::Output>;

    fn parse_value(&self, value: &Value) -> Result<Vec<T::Output>, VldError> {
        let arr = value
            .as_array()
            .ok_or_else(|| crate::primitives::type_error(&None, "array", value))?;

        let mut errors = VldError::new();
        // An inner `Err` may carry no issues: track failure, not issue count.
        let mut failed = false;

        // Length checks
        if let Some(min) = self.min_len {
            if arr.len() < min {
                errors.push_code(IssueCode::TooSmall {
                    origin: SizeOrigin::Array,
                    minimum: min.into(),
                    inclusive: true,
                });
            }
        }

        if let Some(max) = self.max_len {
            if arr.len() > max {
                errors.push_code(IssueCode::TooBig {
                    origin: SizeOrigin::Array,
                    maximum: max.into(),
                    inclusive: true,
                });
            }
        }

        if let Some(exact) = self.exact_len {
            if arr.len() != exact {
                errors.push_code(IssueCode::InvalidLength {
                    origin: SizeOrigin::Array,
                    expected: exact,
                });
            }
        }

        if self.unique {
            let mut seen = std::collections::HashSet::new();
            for (i, item) in arr.iter().enumerate() {
                if !seen.insert(canonical(item)) {
                    errors = errors.merge(
                        VldError::from_code_with_value(IssueCode::NotUnique, item)
                            .with_prefix(PathSegment::Index(i)),
                    );
                }
            }
        }

        if let Some(ref contains) = self.contains {
            let count = arr
                .iter()
                .filter(|v| crate::primitives::json_eq(v, contains))
                .count();
            match self.min_contains {
                None if count == 0 => errors.push_code(IssueCode::MissingContains),
                Some(min_contains) if count < min_contains => {
                    errors.push_code(IssueCode::TooSmall {
                        origin: SizeOrigin::Contains,
                        minimum: min_contains.into(),
                        inclusive: true,
                    })
                }
                _ => {}
            }
            if let Some(max_contains) = self.max_contains {
                if count > max_contains {
                    errors.push_code(IssueCode::TooBig {
                        origin: SizeOrigin::Contains,
                        maximum: max_contains.into(),
                        inclusive: true,
                    });
                }
            }
        }

        // Validate each element
        let mut results = Vec::with_capacity(arr.len());

        for (i, item) in arr.iter().enumerate() {
            match self.element.parse_value(item) {
                Ok(v) => results.push(v),
                Err(e) => {
                    failed = true;
                    errors = errors.merge(e.with_prefix(PathSegment::Index(i)));
                }
            }
        }

        if errors.is_empty() && !failed {
            Ok(results)
        } else {
            Err(errors)
        }
    }
}

#[cfg(feature = "openapi")]
impl<T> crate::openapi::JsonSchema for ZArray<T>
where
    T: VldSchema + crate::openapi::JsonSchema,
{
    fn json_schema(&self) -> serde_json::Value {
        let mut schema = serde_json::json!({
            "type": "array",
            "items": self.element.json_schema(),
        });
        crate::primitives::length_schema(
            &mut schema,
            ("minItems", "maxItems"),
            [self.min_len, self.exact_len].into_iter().flatten(),
            [self.max_len, self.exact_len].into_iter().flatten(),
        );
        if let Some(ref contains) = self.contains {
            schema["contains"] = serde_json::json!({ "const": contains });
            if let Some(min_contains) = self.min_contains {
                schema["minContains"] = serde_json::json!(min_contains);
            }
            if let Some(max_contains) = self.max_contains {
                schema["maxContains"] = serde_json::json!(max_contains);
            }
        }
        if self.unique {
            schema["uniqueItems"] = serde_json::json!(true);
        }
        schema
    }
}

#[cfg(feature = "openapi")]
impl<T> crate::openapi::CollectNestedSchemas for ZArray<T>
where
    T: VldSchema + crate::openapi::CollectNestedSchemas,
{
    fn collect_nested_schemas(&self, out: &mut Vec<crate::schema::NestedSchemaEntry>) {
        self.element.collect_nested_schemas(out);
    }
}
