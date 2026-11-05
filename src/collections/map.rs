use serde_json::Value;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

use crate::error::{IssueCode, PathSegment, VldError};
use crate::schema::VldSchema;

/// Schema for validating a JSON array of `[key, value]` pairs into a `HashMap`.
///
/// Created via [`vld::map()`](crate::map).
///
/// A key that appears twice (after validation) is an error
/// (`duplicate_key`, reported at the later entry's index).
///
/// # Example
/// ```ignore
/// let schema = vld::map(vld::string(), vld::number().int());
/// // Input: [["a", 1], ["b", 2]]
/// // Output: HashMap { "a" => 1, "b" => 2 }
/// ```
pub struct ZMap<K: VldSchema, V: VldSchema> {
    key_schema: K,
    value_schema: V,
}

impl<K: VldSchema, V: VldSchema> ZMap<K, V> {
    pub fn new(key_schema: K, value_schema: V) -> Self {
        Self {
            key_schema,
            value_schema,
        }
    }

    /// Access the key schema.
    pub fn key_schema(&self) -> &K {
        &self.key_schema
    }

    /// Access the value schema.
    pub fn value_schema(&self) -> &V {
        &self.value_schema
    }
}

impl<K, V> VldSchema for ZMap<K, V>
where
    K: VldSchema,
    V: VldSchema,
    K::Output: Eq + std::hash::Hash,
{
    type Output = HashMap<K::Output, V::Output>;

    fn parse_value(&self, value: &Value) -> Result<Self::Output, VldError> {
        let arr = value.as_array().ok_or_else(|| {
            crate::primitives::type_error(&None, "array", value)
        })?;

        let mut result = HashMap::new();
        let mut errors = VldError::new();
        // An inner `Err` may carry no issues: track failure, not issue count.
        let mut failed = false;

        for (i, item) in arr.iter().enumerate() {
            let pair = item.as_array().filter(|a| a.len() == 2);
            match pair {
                Some(pair) => {
                    let k = match self.key_schema.parse_value(&pair[0]) {
                        Ok(k) => Some(k),
                        Err(e) => {
                            failed = true;
                            errors = errors.merge(e.with_prefix(PathSegment::Index(i)));
                            None
                        }
                    };
                    let v = match self.value_schema.parse_value(&pair[1]) {
                        Ok(v) => Some(v),
                        Err(e) => {
                            failed = true;
                            errors = errors.merge(e.with_prefix(PathSegment::Index(i)));
                            None
                        }
                    };
                    if let (Some(k), Some(v)) = (k, v) {
                        match result.entry(k) {
                            Entry::Occupied(_) => {
                                errors = errors.merge(
                                    VldError::from_code_with_value(
                                        IssueCode::DuplicateKey,
                                        &pair[0],
                                    )
                                    .with_prefix(PathSegment::Index(i)),
                                );
                            }
                            Entry::Vacant(e) => {
                                e.insert(v);
                            }
                        }
                    }
                }
                None => {
                    let mut e = VldError::from_code(IssueCode::InvalidMapEntry);
                    e = e.with_prefix(PathSegment::Index(i));
                    errors = errors.merge(e);
                }
            }
        }

        if errors.is_empty() && !failed {
            Ok(result)
        } else {
            Err(errors)
        }
    }
}

/// `[[key, value], ...]`: an array of 2-item tuples.
#[cfg(feature = "openapi")]
impl<K, V> crate::openapi::JsonSchema for ZMap<K, V>
where
    K: VldSchema + crate::openapi::JsonSchema,
    V: VldSchema + crate::openapi::JsonSchema,
{
    fn json_schema(&self) -> Value {
        serde_json::json!({
            "type": "array",
            "items": {
                "type": "array",
                "prefixItems": [self.key_schema.json_schema(), self.value_schema.json_schema()],
                "items": false,
                "minItems": 2,
                "maxItems": 2,
            }
        })
    }
}

#[cfg(feature = "openapi")]
impl<K, V> crate::openapi::CollectNestedSchemas for ZMap<K, V>
where
    K: VldSchema + crate::openapi::CollectNestedSchemas,
    V: VldSchema + crate::openapi::CollectNestedSchemas,
{
    fn collect_nested_schemas(&self, out: &mut Vec<crate::schema::NestedSchemaEntry>) {
        self.key_schema.collect_nested_schemas(out);
        self.value_schema.collect_nested_schemas(out);
    }
}
