use serde_json::Value;
use std::collections::HashMap;

use crate::error::{value_type_name, IssueCode, PathSegment, VldError};
use crate::schema::VldSchema;

/// Schema for validating a JSON array of `[key, value]` pairs into a `HashMap`.
///
/// Created via [`vld::map()`](crate::map).
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
            VldError::single(
                IssueCode::InvalidType {
                    expected: "array".to_string(),
                    received: value_type_name(value),
                },
                format!(
                    "Expected array of [key, value] pairs, received {}",
                    value_type_name(value)
                ),
            )
        })?;

        let mut result = HashMap::new();
        let mut errors = VldError::new();

        for (i, item) in arr.iter().enumerate() {
            let pair = item.as_array().filter(|a| a.len() == 2);
            match pair {
                Some(pair) => {
                    let k = match self.key_schema.parse_value(&pair[0]) {
                        Ok(k) => Some(k),
                        Err(e) => {
                            errors = errors.merge(e.with_prefix(PathSegment::Index(i)));
                            None
                        }
                    };
                    let v = match self.value_schema.parse_value(&pair[1]) {
                        Ok(v) => Some(v),
                        Err(e) => {
                            errors = errors.merge(e.with_prefix(PathSegment::Index(i)));
                            None
                        }
                    };
                    if let (Some(k), Some(v)) = (k, v) {
                        result.insert(k, v);
                    }
                }
                None => {
                    let mut e = VldError::single(
                        IssueCode::Custom {
                            code: "invalid_map_entry".to_string(),
                        },
                        "Each Map entry must be a [key, value] array of length 2",
                    );
                    e = e.with_prefix(PathSegment::Index(i));
                    errors = errors.merge(e);
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
