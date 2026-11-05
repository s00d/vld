use serde_json::{Map, Value};

use crate::error::{IssueCode, PathSegment, SizeOrigin, VldError};
use crate::schema::{DynSchema, VldSchema};

use super::field::{OptionalDynSchema, RequiredDynSchema};

struct ObjectField {
    name: String,
    schema: Box<dyn DynSchema + Send + Sync>,
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

/// Conditional validation rule: when `condition_field` has `condition_value`,
/// validate `target_field` with `schema`.
struct ConditionalRule {
    condition_field: String,
    condition_value: serde_json::Value,
    target_field: String,
    schema: Box<dyn DynSchema + Send + Sync>,
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
pub struct ZObject {
    fields: Vec<ObjectField>,
    unknown_mode: UnknownFieldMode,
    catchall_schema: Option<Box<dyn DynSchema + Send + Sync>>,
    conditional_rules: Vec<ConditionalRule>,
    min_keys: Option<usize>,
    max_keys: Option<usize>,
}

impl ZObject {
    pub fn new() -> Self {
        Self {
            fields: vec![],
            unknown_mode: UnknownFieldMode::Strip,
            catchall_schema: None,
            conditional_rules: vec![],
            min_keys: None,
            max_keys: None,
        }
    }

    /// Add a field with its validation schema.
    ///
    /// With the `openapi` feature the field keeps its JSON Schema, its
    /// `required` status (`.optional()` / `.nullish()` / `.with_default(...)`
    /// / `.nullable()` are not required) and its named nested schemas.
    pub fn field<S: DynSchema + Send + Sync + 'static>(
        mut self,
        name: impl Into<String>,
        schema: S,
    ) -> Self {
        self.fields.push(ObjectField {
            name: name.into(),
            schema: Box::new(schema),
        });
        self
    }

    /// Same as [`field()`](Self::field), which now keeps the JSON Schema too.
    #[cfg(feature = "openapi")]
    #[deprecated(since = "0.5.0", note = "use `field()`: it keeps the JSON Schema now")]
    pub fn field_schema<S: DynSchema + Send + Sync + 'static>(
        self,
        name: impl Into<String>,
        schema: S,
    ) -> Self {
        self.field(name, schema)
    }

    /// Add a field that is automatically optional (null/missing → `null`).
    ///
    /// The field won't cause a validation error if it is missing or null.
    /// When the value is missing/null the inner schema still runs first, so
    /// `.with_default(...)` and `.catch(...)` produce their value; otherwise
    /// the output is `null`.
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
    pub fn field_optional<S: DynSchema + Send + Sync + 'static>(
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
    pub fn catchall<S: DynSchema + Send + Sync + 'static>(mut self, schema: S) -> Self {
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
    pub fn when<S: DynSchema + Send + Sync + 'static>(
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

    /// Require at least this many keys in the input object.
    pub fn min_keys(mut self, n: usize) -> Self {
        self.min_keys = Some(n);
        self
    }

    /// Require at most this many keys in the input object.
    pub fn max_keys(mut self, n: usize) -> Self {
        self.max_keys = Some(n);
        self
    }

    /// Same as [`partial()`](Self::partial): **not** recursive.
    ///
    /// Fields are type-erased, so nested objects cannot be reached; call
    /// `partial()` on nested [`ZObject`]s yourself before adding them.
    #[deprecated(
        since = "0.5.0",
        note = "not recursive; identical to `partial()`. Call `partial()` on nested objects explicitly."
    )]
    pub fn deep_partial(self) -> Self {
        self.partial()
    }

    /// Get the list of field names defined in this schema.
    pub fn keyof(&self) -> Vec<String> {
        self.fields.iter().map(|f| f.name.clone()).collect()
    }
}

impl ZObject {
    /// JSON Schema of this object; same as
    /// [`JsonSchema::json_schema`](crate::openapi::JsonSchema::json_schema).
    #[cfg(feature = "openapi")]
    pub fn to_json_schema(&self) -> Value {
        crate::openapi::JsonSchema::json_schema(self)
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
        let obj = crate::error::expect_object(value)?;

        let mut result = Map::new();
        let mut errors = VldError::new();

        if let Some(min) = self.min_keys {
            if obj.len() < min {
                errors.push_code(IssueCode::TooSmall {
                    origin: SizeOrigin::Object,
                    minimum: min.into(),
                    inclusive: true,
                });
            }
        }
        if let Some(max) = self.max_keys {
            if obj.len() > max {
                errors.push_code(IssueCode::TooBig {
                    origin: SizeOrigin::Object,
                    maximum: max.into(),
                    inclusive: true,
                });
            }
        }

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
        let known_keys: std::collections::HashSet<&str> =
            self.fields.iter().map(|f| f.name.as_str()).collect();
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

/// Properties from the fields (with their own schemas and `required` status);
/// `additionalProperties` is `false` for `strict()`, the catchall schema when
/// set, `true` otherwise.
#[cfg(feature = "openapi")]
impl crate::openapi::JsonSchema for ZObject {
    fn json_schema(&self) -> Value {
        let required: Vec<&str> = self
            .fields
            .iter()
            .filter(|f| f.schema.dyn_is_required())
            .map(|f| f.name.as_str())
            .collect();
        let props: Map<String, Value> = self
            .fields
            .iter()
            .map(|f| (f.name.clone(), f.schema.dyn_json_schema()))
            .collect();
        let mut schema = serde_json::json!({
            "type": "object",
            "properties": props,
            "additionalProperties": self.unknown_mode != UnknownFieldMode::Strict,
        });
        if !required.is_empty() {
            schema["required"] = serde_json::json!(required);
        }
        if let Some(ref catchall) = self.catchall_schema {
            schema["additionalProperties"] = catchall.dyn_json_schema();
        }
        if let Some(min) = self.min_keys {
            schema["minProperties"] = serde_json::json!(min);
        }
        if let Some(max) = self.max_keys {
            schema["maxProperties"] = serde_json::json!(max);
        }
        schema
    }
}

#[cfg(feature = "openapi")]
impl crate::openapi::CollectNestedSchemas for ZObject {
    fn collect_nested_schemas(&self, out: &mut Vec<crate::schema::NestedSchemaEntry>) {
        let fields = self.fields.iter().map(|f| &f.schema);
        let rules = self.conditional_rules.iter().map(|r| &r.schema);
        for s in fields.chain(rules).chain(self.catchall_schema.iter()) {
            s.dyn_collect_nested_schemas(out);
        }
    }
}
