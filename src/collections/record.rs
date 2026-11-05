use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::marker::PhantomData;

use crate::error::{IssueCode, PathSegment, SizeOrigin, VldError};
use crate::schema::VldSchema;

/// How a record turns a JSON object key into an output key: [`AnyKey`]
/// (plain `String`) or [`KeySchema`] (validated by a schema).
pub trait RecordKey {
    /// Output key type.
    type Output;
    /// Parse one object key.
    fn parse_key(&self, key: &str) -> Result<Self::Output, VldError>;
}

/// Record keys taken as is (`String`, no validation). Default of [`ZRecord`].
#[derive(Clone, Copy, Default)]
pub struct AnyKey;

impl RecordKey for AnyKey {
    type Output = String;

    fn parse_key(&self, key: &str) -> Result<String, VldError> {
        Ok(key.to_owned())
    }
}

/// Record keys validated by a schema, which receives each key as a JSON
/// string. Created by [`vld::record_with_keys()`](crate::record_with_keys).
#[derive(Clone)]
pub struct KeySchema<K>(pub K);

impl<K: VldSchema> RecordKey for KeySchema<K> {
    type Output = K::Output;

    fn parse_key(&self, key: &str) -> Result<K::Output, VldError> {
        self.0.parse_value(&Value::String(key.to_owned()))
    }
}

/// Schema for validating JSON objects as key-value records.
/// Created via [`vld::record()`](crate::record) (`HashMap<String, V>`) or
/// [`vld::record_with_keys()`](crate::record_with_keys) (`HashMap<K, V>`);
/// [`ordered()`](Self::ordered) switches the output to a `BTreeMap`.
///
/// Values are validated against the value schema; key and value issues are
/// reported at the key's path. If two keys parse to the same output key, the
/// later one wins.
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// let schema = vld::record(vld::number().int().positive());
/// let result = schema.parse(r#"{"a": 1, "b": 2}"#).unwrap();
/// assert_eq!(result.get("a"), Some(&1));
///
/// let ordered = vld::record(vld::boolean()).ordered().parse(r#"{"b": true, "a": false}"#).unwrap();
/// assert_eq!(ordered.keys().collect::<Vec<_>>(), ["a", "b"]);
/// ```
pub struct ZRecord<V: VldSchema, K = AnyKey, M = HashMap<String, <V as VldSchema>::Output>> {
    key_schema: K,
    value_schema: V,
    min_keys: Option<usize>,
    max_keys: Option<usize>,
    _map: PhantomData<fn() -> M>,
}

/// Record with `BTreeMap` output (sorted keys); see [`ZRecord::ordered`].
pub type ZOrderedRecord<V, K = AnyKey> =
    ZRecord<V, K, BTreeMap<<K as RecordKey>::Output, <V as VldSchema>::Output>>;

impl<V: VldSchema> ZRecord<V> {
    pub fn new(value_schema: V) -> Self {
        Self::with_keys(AnyKey, value_schema)
    }
}

impl<V: VldSchema, K, M> ZRecord<V, K, M> {
    pub(crate) fn with_keys(key_schema: K, value_schema: V) -> Self {
        Self {
            key_schema,
            value_schema,
            min_keys: None,
            max_keys: None,
            _map: PhantomData,
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

    /// Produce a `BTreeMap` (sorted by key) instead of a `HashMap`.
    pub fn ordered(self) -> ZOrderedRecord<V, K>
    where
        K: RecordKey,
        K::Output: Ord,
    {
        ZRecord {
            key_schema: self.key_schema,
            value_schema: self.value_schema,
            min_keys: self.min_keys,
            max_keys: self.max_keys,
            _map: PhantomData,
        }
    }
}

impl<V, K, M> VldSchema for ZRecord<V, K, M>
where
    V: VldSchema,
    K: RecordKey,
    M: Default + Extend<(K::Output, V::Output)>,
{
    type Output = M;

    fn parse_value(&self, value: &Value) -> Result<M, VldError> {
        let obj = value
            .as_object()
            .ok_or_else(|| crate::primitives::type_error(&None, "object", value))?;

        let mut errors = VldError::new();
        // An inner `Err` may carry no issues: track failure, not issue count.
        let mut failed = false;

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

        let mut result = M::default();

        for (key, val) in obj {
            let k = self.key_schema.parse_key(key);
            let v = self.value_schema.parse_value(val);
            match (k, v) {
                (Ok(k), Ok(v)) => result.extend(std::iter::once((k, v))),
                (k, v) => {
                    failed = true;
                    for e in [k.err(), v.err()].into_iter().flatten() {
                        errors = errors.merge(e.with_prefix(PathSegment::Field(key.clone())));
                    }
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

#[cfg(feature = "openapi")]
impl crate::openapi::JsonSchema for AnyKey {
    fn json_schema(&self) -> Value {
        serde_json::json!({})
    }
}

#[cfg(feature = "openapi")]
impl<K: crate::openapi::JsonSchema> crate::openapi::JsonSchema for KeySchema<K> {
    fn json_schema(&self) -> Value {
        self.0.json_schema()
    }
}

/// `{"type": "object", "additionalProperties": <value>}`; a key schema adds
/// `propertyNames`.
#[cfg(feature = "openapi")]
impl<V, K, M> crate::openapi::JsonSchema for ZRecord<V, K, M>
where
    V: VldSchema + crate::openapi::JsonSchema,
    K: crate::openapi::JsonSchema,
{
    fn json_schema(&self) -> Value {
        let mut schema = serde_json::json!({
            "type": "object",
            "additionalProperties": self.value_schema.json_schema(),
        });
        let keys = self.key_schema.json_schema();
        if keys.as_object().is_some_and(|o| !o.is_empty()) {
            schema["propertyNames"] = keys;
        }
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
impl<V, K, M> crate::openapi::CollectNestedSchemas for ZRecord<V, K, M>
where
    V: VldSchema + crate::openapi::CollectNestedSchemas,
{
    fn collect_nested_schemas(&self, out: &mut Vec<crate::schema::NestedSchemaEntry>) {
        self.value_schema.collect_nested_schemas(out);
    }
}
