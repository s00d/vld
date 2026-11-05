use serde_json::Value;
use std::collections::HashMap;

use crate::error::{value_type_name, IssueCode, PathSegment, SizeOrigin, VldError};
use crate::schema::VldSchema;

/// Schema for validating JSON objects as key-value records.
/// Created via [`vld::record()`](crate::record).
///
/// All keys are strings (JSON constraint). Values are validated against the inner schema.
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// let schema = vld::record(vld::number().int().positive());
/// let result = schema.parse(r#"{"a": 1, "b": 2}"#).unwrap();
/// assert_eq!(result.get("a"), Some(&1));
/// ```
pub struct ZRecord<V: VldSchema> {
    value_schema: V,
    min_keys: Option<usize>,
    max_keys: Option<usize>,
}

impl<V: VldSchema> ZRecord<V> {
    pub fn new(value_schema: V) -> Self {
        Self {
            value_schema,
            min_keys: None,
            max_keys: None,
        }
    }

    /// Minimum number of keys.
    pub fn min_keys(mut self, n: usize) -> Self {
        self.min_keys = Some(n);
        self
    }

    /// Maximum number of keys.
    pub fn max_keys(mut self, n: usize) -> Self {
        self.max_keys = Some(n);
        self
    }
}

impl<V: VldSchema> VldSchema for ZRecord<V> {
    type Output = HashMap<String, V::Output>;

    fn parse_value(&self, value: &Value) -> Result<HashMap<String, V::Output>, VldError> {
        let obj = value.as_object().ok_or_else(|| {
            VldError::from_code(IssueCode::InvalidType {
                expected: "object".to_string(),
                received: value_type_name(value),
            })
        })?;

        let mut errors = VldError::new();

        if let Some(min) = self.min_keys {
            if obj.len() < min {
                errors.push_code(IssueCode::TooSmall {
                    origin: SizeOrigin::Record,
                    minimum: min.into(),
                    inclusive: true,
                });
            }
        }

        if let Some(max) = self.max_keys {
            if obj.len() > max {
                errors.push_code(IssueCode::TooBig {
                    origin: SizeOrigin::Record,
                    maximum: max.into(),
                    inclusive: true,
                });
            }
        }

        let mut result = HashMap::new();

        for (key, val) in obj {
            match self.value_schema.parse_value(val) {
                Ok(v) => {
                    result.insert(key.clone(), v);
                }
                Err(e) => {
                    errors = errors.merge(e.with_prefix(PathSegment::Field(key.clone())));
                }
            }
        }

        if errors.is_empty() {
            Ok(result)
        } else {
            Err(errors)
        }
    }
}

#[cfg(feature = "openapi")]
impl<V> crate::openapi::JsonSchema for ZRecord<V>
where
    V: VldSchema + crate::openapi::JsonSchema,
{
    fn json_schema(&self) -> serde_json::Value {
        let mut schema = serde_json::json!({
            "type": "object",
            "additionalProperties": self.value_schema.json_schema(),
        });
        crate::primitives::length_schema(
            &mut schema,
            ("minProperties", "maxProperties"),
            self.min_keys,
            self.max_keys,
        );
        schema
    }
}

#[cfg(feature = "openapi")]
impl<V> crate::openapi::CollectNestedSchemas for ZRecord<V>
where
    V: VldSchema + crate::openapi::CollectNestedSchemas,
{
    fn collect_nested_schemas(&self, out: &mut Vec<crate::schema::NestedSchemaEntry>) {
        self.value_schema.collect_nested_schemas(out);
    }
}
