use serde_json::Value;
use std::ops::RangeInclusive;

use crate::error::{IssueCode, PathSegment, VldError, absent_field_error};
use crate::schema::VldSchema;

/// Schema for an ordered pair `{"start": a, "end": b}` with `a <= b`, into
/// `RangeInclusive<T>`. Created via [`vld::range()`](crate::range).
///
/// Both fields are parsed with the item schema (any date backend's
/// `date()` / `datetime()` / `time()`, numbers, decimals, durations, …:
/// anything whose output is `PartialOrd`), and compared as parsed values —
/// `2024-01-01T10:00:00+02:00` is before `2024-01-01T09:00:00Z`. When both
/// parse and the end is before the start, the issue is `range_order` at the
/// end field. Equal ends are allowed.
///
/// ```
/// use vld::prelude::*;
///
/// let r = vld::range(vld::number().int()).parse(r#"{"start": 1, "end": 5}"#).unwrap();
/// assert_eq!(r, 1..=5);
///
/// let stay = vld::range(vld::u32()).keys("from", "to");
/// let err = stay.parse(r#"{"from": 7, "to": 3}"#).unwrap_err();
/// assert_eq!(err.issues[0].path_string(), ".to");
/// assert_eq!(err.issues[0].code.key(), "range_order");
/// ```
#[derive(Clone)]
pub struct ZRange<S> {
    item: S,
    start: String,
    end: String,
    messages: crate::primitives::Messages,
}

impl<S: VldSchema> ZRange<S> {
    /// Create the schema (the same as its `vld::` constructor function).
    pub fn new(item: S) -> Self {
        Self {
            item,
            start: "start".to_string(),
            end: "end".to_string(),
            messages: None,
        }
    }

    /// Field names of the two ends (default `"start"` / `"end"`).
    pub fn keys(mut self, start: impl Into<String>, end: impl Into<String>) -> Self {
        self.start = start.into();
        self.end = end.into();
        self
    }

    /// Override the message by check key: `"range_order"`.
    pub fn with_messages<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> Option<String> + Send + Sync + 'static,
    {
        self.messages = Some(std::sync::Arc::new(f));
        self
    }

    fn field(
        &self,
        obj: &serde_json::Map<String, Value>,
        key: &str,
    ) -> Result<S::Output, VldError> {
        match obj.get(key) {
            Some(v) => self.item.parse_value(v),
            None => self.item.parse_missing().map_err(absent_field_error),
        }
        .map_err(|e| e.with_prefix(PathSegment::Field(key.to_string())))
    }
}

impl<S: VldSchema> VldSchema for ZRange<S>
where
    S::Output: PartialOrd,
{
    type Output = RangeInclusive<S::Output>;

    fn parse_value(&self, value: &Value) -> Result<Self::Output, VldError> {
        let obj = crate::error::expect_object(value)?;
        match (self.field(obj, &self.start), self.field(obj, &self.end)) {
            (Ok(start), Ok(end)) if end < start => {
                let code = IssueCode::RangeOrder {
                    start: self.start.clone(),
                    end: self.end.clone(),
                };
                let msg = self.messages.as_ref().and_then(|f| f("range_order"));
                let received = obj.get(&self.end).unwrap_or(&Value::Null);
                Err(crate::error::issue_error(code, msg.as_ref(), received)
                    .with_prefix(PathSegment::Field(self.end.clone())))
            }
            (Ok(start), Ok(end)) => Ok(start..=end),
            (a, b) => Err(a
                .err()
                .unwrap_or_default()
                .merge(b.err().unwrap_or_default())),
        }
    }
}

/// `{"type": "object", "properties": {start: item, end: item}, "required":
/// [start, end], "x-rangeOrder": [start, end]}` (`x-rangeOrder` is a vld
/// extension: the order is not expressible in JSON Schema; `vld::fake`
/// honours it).
#[cfg(feature = "openapi")]
impl<S: crate::openapi::JsonSchema> crate::openapi::JsonSchema for ZRange<S> {
    fn json_schema(&self) -> Value {
        let item = self.item.json_schema();
        let mut props = serde_json::Map::new();
        props.insert(self.start.clone(), item.clone());
        props.insert(self.end.clone(), item);
        serde_json::json!({
            "type": "object",
            "properties": props,
            "required": [self.start, self.end],
            "x-rangeOrder": [self.start, self.end],
        })
    }
}

#[cfg(feature = "openapi")]
impl<S: crate::openapi::CollectNestedSchemas> crate::openapi::CollectNestedSchemas for ZRange<S> {
    fn collect_nested_schemas(&self, out: &mut Vec<crate::schema::NestedSchemaEntry>) {
        self.item.collect_nested_schemas(out)
    }
}
