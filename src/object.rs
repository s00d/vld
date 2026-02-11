use serde_json::{Map, Value};

use crate::error::{value_type_name, IssueCode, PathSegment, VldError};
use crate::schema::VldSchema;

/// Object-safe trait for type-erased schema validation.
///
/// Used internally by [`ZObject`] to store heterogeneous field schemas.
pub trait DynSchema {
    fn dyn_parse(&self, value: &Value) -> Result<Value, VldError>;
    /// Generate a JSON Schema for this field. Returns empty schema `{}` by default.
    ///
    /// Only available with the `openapi` feature.
    #[cfg(feature = "openapi")]
    fn dyn_json_schema(&self) -> Value {
        serde_json::json!({})
    }
}

/// Blanket implementation: any `VldSchema` whose output is `Serialize`
/// can be used as a dynamic schema.
impl<T> DynSchema for T
where
    T: VldSchema,
    T::Output: serde::Serialize,
{
    fn dyn_parse(&self, value: &Value) -> Result<Value, VldError> {
        let result = self.parse_value(value)?;
        serde_json::to_value(&result).map_err(|e| {
            VldError::single(
                IssueCode::Custom {
                    code: "serialize".to_string(),
                },
                format!("Failed to serialize validated value: {}", e),
            )
        })
    }
}

#[cfg(feature = "openapi")]
/// Wrapper that stores a schema implementing both `DynSchema` and `JsonSchema`.
///
/// Used by [`ZObject::field_schema()`] to include JSON Schema info for fields.
pub(crate) struct JsonSchemaField<T> {
    inner: T,
}

#[cfg(feature = "openapi")]
impl<T> DynSchema for JsonSchemaField<T>
where
    T: VldSchema + crate::json_schema::JsonSchema,
    T::Output: serde::Serialize,
{
    fn dyn_parse(&self, value: &Value) -> Result<Value, VldError> {
        let result = self.inner.parse_value(value)?;
        serde_json::to_value(&result).map_err(|e| {
            VldError::single(
                IssueCode::Custom {
                    code: "serialize".to_string(),
                },
                format!("Failed to serialize validated value: {}", e),
            )
        })
    }

    fn dyn_json_schema(&self) -> Value {
        self.inner.json_schema()
    }
}

struct ObjectField {
    name: String,
    schema: Box<dyn DynSchema>,
}

/// How to handle unknown fields not declared in the schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnknownFieldMode {
    /// Silently drop unknown fields from the output (default).
    Strip,
    /// Reject unknown fields with a validation error.
    Strict,
    /// Keep unknown fields as-is in the output.
    Passthrough,
}

/// Dynamic object schema with runtime-defined fields.
///
/// For compile-time type-safe objects, use the [`schema!`](crate::schema!) macro instead.
///
/// # Unknown field handling
///
/// - **`strip()`** (default) — unknown fields are silently removed from the output.
/// - **`strict()`** — unknown fields cause a validation error.
/// - **`passthrough()`** — unknown fields are kept as-is in the output.
///
/// # Example
/// ```
/// use vld::prelude::*;
///
/// let schema = vld::object()
///     .field("name", vld::string().min(1))
///     .field("age", vld::number().int().min(0));
/// ```
/// Conditional validation rule: when `condition_field` has `condition_value`,
/// validate `target_field` with `schema`.
struct ConditionalRule {
    condition_field: String,
    condition_value: serde_json::Value,
    target_field: String,
    schema: Box<dyn DynSchema>,
}

pub struct ZObject {
    fields: Vec<ObjectField>,
    unknown_mode: UnknownFieldMode,
    catchall_schema: Option<Box<dyn DynSchema>>,
    conditional_rules: Vec<ConditionalRule>,
}

impl ZObject {
    pub fn new() -> Self {
        Self {
            fields: vec![],
            unknown_mode: UnknownFieldMode::Strip,
            catchall_schema: None,
            conditional_rules: vec![],
        }
    }

    /// Add a field with its validation schema.
    pub fn field<S: DynSchema + 'static>(mut self, name: impl Into<String>, schema: S) -> Self {
        self.fields.push(ObjectField {
            name: name.into(),
            schema: Box::new(schema),
        });
        self
    }

    /// Add a field with its validation schema **and** JSON Schema support.
    ///
    /// Same as [`field()`](Self::field), but the field's schema will be
    /// included in the output of [`to_json_schema()`](Self::to_json_schema)
    /// and [`json_schema()`](crate::json_schema::JsonSchema::json_schema).
    ///
    /// Requires the `openapi` feature.
    #[cfg(feature = "openapi")]
    pub fn field_schema<S>(mut self, name: impl Into<String>, schema: S) -> Self
    where
        S: VldSchema + crate::json_schema::JsonSchema + 'static,
        S::Output: serde::Serialize,
    {
        self.fields.push(ObjectField {
            name: name.into(),
            schema: Box::new(JsonSchemaField { inner: schema }),
        });
        self
    }

    /// Add a field that is automatically optional (null/missing → `null`).
    ///
    /// Shorthand for `.field(name, OptionalDynSchema(schema))` — the field won't
    /// cause a validation error if it is missing or null.
    ///
    /// # Example
    /// ```
    /// use vld::prelude::*;
    ///
    /// let schema = vld::object()
    ///     .field("name", vld::string().min(1))
    ///     .field_optional("nickname", vld::string().min(1));
    ///
    /// let result = schema.parse(r#"{"name": "Alice"}"#).unwrap();
    /// assert_eq!(result.get("nickname").unwrap(), &serde_json::Value::Null);
    /// ```
    pub fn field_optional<S: DynSchema + 'static>(
        mut self,
        name: impl Into<String>,
        schema: S,
    ) -> Self {
        self.fields.push(ObjectField {
            name: name.into(),
            schema: Box::new(OptionalDynSchema(Box::new(schema))),
        });
        self
    }

    /// Reject unknown fields not defined in the schema.
    pub fn strict(mut self) -> Self {
        self.unknown_mode = UnknownFieldMode::Strict;
        self
    }

    /// Silently remove unknown fields from the output (default behavior).
    pub fn strip(mut self) -> Self {
        self.unknown_mode = UnknownFieldMode::Strip;
        self
    }

    /// Keep unknown fields as-is in the output without validation.
    pub fn passthrough(mut self) -> Self {
        self.unknown_mode = UnknownFieldMode::Passthrough;
        self
    }

    /// Remove a field definition by name. Returns self for chaining.
    ///
    /// Useful with [`extend()`](Self::extend) to override fields.
    pub fn omit(mut self, name: &str) -> Self {
        self.fields.retain(|f| f.name != name);
        self
    }

    /// Keep only the listed fields, removing all others.
    pub fn pick(mut self, names: &[&str]) -> Self {
        self.fields.retain(|f| names.contains(&f.name.as_str()));
        self
    }

    /// Merge another object schema's fields into this one.
    ///
    /// If both schemas define the same field, the one from `other` wins.
    pub fn extend(mut self, other: ZObject) -> Self {
        for field in other.fields {
            self.fields.retain(|f| f.name != field.name);
            self.fields.push(field);
        }
        self
    }

    /// Alias for [`extend`](Self::extend).
    pub fn merge(self, other: ZObject) -> Self {
        self.extend(other)
    }

    /// Make all fields optional: null/missing values return `null` in the output
    /// instead of failing validation.
    ///
    /// Equivalent to Zod's `.partial()`.
    pub fn partial(mut self) -> Self {
        self.fields = self
            .fields
            .into_iter()
            .map(|f| ObjectField {
                name: f.name,
                schema: Box::new(OptionalDynSchema(f.schema)),
            })
            .collect();
        self
    }

    /// Make all fields required: null values will fail validation.
    /// This is the opposite of [`partial()`](Self::partial).
    pub fn required(mut self) -> Self {
        self.fields = self
            .fields
            .into_iter()
            .map(|f| ObjectField {
                name: f.name,
                schema: Box::new(RequiredDynSchema(f.schema)),
            })
            .collect();
        self
    }

    /// Validate unknown fields using the given schema instead of stripping/rejecting them.
    ///
    /// When set, unknown fields are parsed through the catchall schema
    /// regardless of the unknown field mode.
    pub fn catchall<S: DynSchema + 'static>(mut self, schema: S) -> Self {
        self.catchall_schema = Some(Box::new(schema));
        self
    }

    /// Add a conditional validation rule.
    ///
    /// When `condition_field` has the given value, `target_field` is validated
    /// with the provided schema **in addition** to any existing field schemas.
    ///
    /// # Example
    /// ```
    /// use vld::prelude::*;
    ///
    /// let schema = vld::object()
    ///     .field("role", vld::string())
    ///     .field_optional("admin_key", vld::string())
    ///     .when("role", "admin", "admin_key", vld::string().min(10));
    ///
    /// // When role != "admin", admin_key is optional and any value is fine
    /// let ok = schema.parse(r#"{"role": "user"}"#);
    /// assert!(ok.is_ok());
    ///
    /// // When role == "admin", admin_key must pass the extra schema
    /// let err = schema.parse(r#"{"role": "admin", "admin_key": "short"}"#);
    /// assert!(err.is_err());
    /// ```
    pub fn when<S: DynSchema + 'static>(
        mut self,
        condition_field: impl Into<String>,
        condition_value: impl Into<serde_json::Value>,
        target_field: impl Into<String>,
        schema: S,
    ) -> Self {
        self.conditional_rules.push(ConditionalRule {
            condition_field: condition_field.into(),
            condition_value: condition_value.into(),
            target_field: target_field.into(),
            schema: Box::new(schema),
        });
        self
    }

    /// Make all fields optional recursively.
    ///
    /// Currently equivalent to [`partial()`](Self::partial) — nested objects
    /// must apply `partial()` separately.
    pub fn deep_partial(self) -> Self {
        self.partial()
    }

    /// Get the list of field names defined in this schema.
    pub fn keyof(&self) -> Vec<String> {
        self.fields.iter().map(|f| f.name.clone()).collect()
    }

    /// Generate a JSON Schema representation of this object schema.
    ///
    /// Fields added via [`field_schema()`](Self::field_schema) will include their
    /// full JSON Schema. Fields added via [`field()`](Self::field) will appear as
    /// empty schemas `{}`.
    ///
    /// Requires the `openapi` feature.
    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> serde_json::Value {
        let required: Vec<String> = self.fields.iter().map(|f| f.name.clone()).collect();
        let mut props = serde_json::Map::new();
        for f in &self.fields {
            props.insert(f.name.clone(), f.schema.dyn_json_schema());
        }
        let mut schema = serde_json::json!({
            "type": "object",
            "required": required,
            "properties": Value::Object(props),
            "additionalProperties": self.unknown_mode != UnknownFieldMode::Strict,
        });
        if let Some(ref catchall) = self.catchall_schema {
            schema["additionalProperties"] = catchall.dyn_json_schema();
        }
        schema
    }
}

impl Default for ZObject {
    fn default() -> Self {
        Self::new()
    }
}

impl VldSchema for ZObject {
    type Output = Map<String, Value>;

    fn parse_value(&self, value: &Value) -> Result<Map<String, Value>, VldError> {
        let obj = value.as_object().ok_or_else(|| {
            VldError::single(
                IssueCode::InvalidType {
                    expected: "object".to_string(),
                    received: value_type_name(value),
                },
                format!("Expected object, received {}", value_type_name(value)),
            )
        })?;

        let mut result = Map::new();
        let mut errors = VldError::new();

        // Validate defined fields
        for field in &self.fields {
            let field_value = obj.get(&field.name).unwrap_or(&Value::Null);
            match field.schema.dyn_parse(field_value) {
                Ok(v) => {
                    result.insert(field.name.clone(), v);
                }
                Err(e) => {
                    errors = errors.merge(e.with_prefix(PathSegment::Field(field.name.clone())));
                }
            }
        }

        // Handle unknown fields
        let known_keys: Vec<&str> = self.fields.iter().map(|f| f.name.as_str()).collect();
        let unknown_keys: Vec<&String> = obj
            .keys()
            .filter(|k| !known_keys.contains(&k.as_str()))
            .collect();

        if let Some(catchall) = &self.catchall_schema {
            for key in &unknown_keys {
                let val = &obj[key.as_str()];
                match catchall.dyn_parse(val) {
                    Ok(v) => {
                        result.insert((*key).clone(), v);
                    }
                    Err(e) => {
                        errors = errors.merge(e.with_prefix(PathSegment::Field((*key).clone())));
                    }
                }
            }
        } else {
            match self.unknown_mode {
                UnknownFieldMode::Strip => {}
                UnknownFieldMode::Strict => {
                    for key in &unknown_keys {
                        let mut issue_err = VldError::single(
                            IssueCode::UnrecognizedField,
                            format!("Unrecognized field: \"{}\"", key),
                        );
                        issue_err = issue_err.with_prefix(PathSegment::Field((*key).clone()));
                        errors = errors.merge(issue_err);
                    }
                }
                UnknownFieldMode::Passthrough => {
                    for key in &unknown_keys {
                        result.insert((*key).clone(), obj[key.as_str()].clone());
                    }
                }
            }
        }

        // Evaluate conditional rules
        for rule in &self.conditional_rules {
            let cond_val = obj.get(&rule.condition_field).unwrap_or(&Value::Null);
            if *cond_val == rule.condition_value {
                let target_val = obj.get(&rule.target_field).unwrap_or(&Value::Null);
                if let Err(e) = rule.schema.dyn_parse(target_val) {
                    errors =
                        errors.merge(e.with_prefix(PathSegment::Field(rule.target_field.clone())));
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

/// Internal wrapper that makes a DynSchema nullable (null/missing → Value::Null).
struct OptionalDynSchema(Box<dyn DynSchema>);

impl DynSchema for OptionalDynSchema {
    fn dyn_parse(&self, value: &Value) -> Result<Value, VldError> {
        if value.is_null() {
            return Ok(Value::Null);
        }
        self.0.dyn_parse(value)
    }
}

/// Internal wrapper that rejects null values.
struct RequiredDynSchema(Box<dyn DynSchema>);

impl DynSchema for RequiredDynSchema {
    fn dyn_parse(&self, value: &Value) -> Result<Value, VldError> {
        if value.is_null() {
            return Err(VldError::single(
                IssueCode::MissingField,
                "Required field is missing or null",
            ));
        }
        self.0.dyn_parse(value)
    }
}
