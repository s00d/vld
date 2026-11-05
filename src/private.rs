//! Runtime helpers shared by `schema!`, `impl_validate_fields!` and
//! `#[derive(Validate)]`. Not public API: may change in any release.

use serde_json::{Map, Value};
use std::any::Any;
use std::sync::OnceLock;

use crate::error::{PathSegment, VldError};
use crate::object::FieldResult;
use crate::schema::VldSchema;

/// Lazily built, process-wide schema used by `schema!` / `#[derive(Validate)]`
/// so field schemas (and their regexes) are built once, not on every parse.
///
/// One cell per schema expression (a `static` inside the generated code);
/// never used in generic code, where a `static` would be shared between
/// instantiations.
pub struct SchemaCell(OnceLock<Box<dyn Any + Send + Sync>>);

impl SchemaCell {
    #[allow(clippy::new_without_default)]
    pub const fn new() -> Self {
        Self(OnceLock::new())
    }

    /// The cached schema, built by `init` on first use.
    pub fn get<T: Any + Send + Sync>(&'static self, init: impl FnOnce() -> T) -> &'static T {
        self.0
            .get_or_init(|| Box::new(init()))
            .downcast_ref::<T>()
            .expect("vld: one SchemaCell per schema expression")
    }
}

/// Lazily computed JSON Schema of a non-generic `schema!` / derive struct.
pub type JsonSchemaCell = OnceLock<Value>;

/// Return the value as a JSON object, or an `invalid_type` error.
pub fn expect_object(value: &Value) -> Result<&Map<String, Value>, VldError> {
    crate::error::expect_object(value)
}

static NULL: Value = Value::Null;

/// JSON key for a field identifier: strips the raw-identifier prefix (`r#type` → `type`).
pub fn field_key(ident: &'static str) -> &'static str {
    ident.strip_prefix("r#").unwrap_or(ident)
}

/// Value of `key`, or `null` when missing.
pub fn field_value<'a>(obj: &'a Map<String, Value>, key: &str) -> &'a Value {
    obj.get(key).unwrap_or(&NULL)
}

/// Parse the field `key`; a required field whose key is absent fails with
/// `missing_field` (a present `null` keeps `invalid_type`).
fn parse_key<S: VldSchema>(
    obj: &Map<String, Value>,
    key: &str,
    schema: &S,
) -> Result<S::Output, VldError> {
    schema.parse_value(field_value(obj, key)).map_err(|e| {
        if obj.contains_key(key) {
            e
        } else {
            crate::error::absent_field_error(e)
        }
    })
}

/// Parse one field; on failure push its issues (prefixed with `key`) to `errors`.
pub fn parse_field<S: VldSchema>(
    obj: &Map<String, Value>,
    key: &str,
    schema: &S,
    errors: &mut VldError,
) -> Option<S::Output> {
    match parse_key(obj, key, schema) {
        Ok(v) => Some(v),
        Err(e) => {
            let e = e.with_prefix(PathSegment::Field(key.to_string()));
            errors.issues.extend(e.issues);
            None
        }
    }
}

/// Validate one field into a [`FieldResult`] (output serialized to JSON).
pub fn field_result<S>(obj: &Map<String, Value>, key: &str, schema: &S) -> FieldResult
where
    S: VldSchema,
    S::Output: serde::Serialize,
{
    let input = field_value(obj, key);
    FieldResult {
        name: key.to_string(),
        input: input.clone(),
        result: parse_key(obj, key, schema).and_then(|v| crate::schema::to_json_value(&v)),
    }
}

/// Parse one field leniently: record a [`FieldResult`], fall back to `Default` on error.
pub fn lenient_field<S>(
    obj: &Map<String, Value>,
    key: &str,
    schema: &S,
    results: &mut Vec<FieldResult>,
) -> S::Output
where
    S: VldSchema,
    S::Output: serde::Serialize + Default,
{
    let input = field_value(obj, key);
    let (out, result) = match parse_key(obj, key, schema) {
        Ok(v) => {
            let repr = serde_json::to_value(&v).unwrap_or_else(|_| input.clone());
            (v, Ok(repr))
        }
        Err(e) => (Default::default(), Err(e)),
    };
    results.push(FieldResult {
        name: key.to_string(),
        input: input.clone(),
        result,
    });
    out
}

/// Serialize a value for `validate()` / `is_valid()`.
pub fn to_json<T: serde::Serialize + ?Sized>(instance: &T) -> Result<Value, VldError> {
    serde_json::to_value(instance).map_err(|e| {
        VldError::from_code(crate::error::IssueCode::SerializeError {
            detail: e.to_string(),
        })
    })
}

/// Sanitize a Rust type string into a valid OpenAPI component name
/// (`[A-Za-z0-9._-]`): whitespace dropped, other characters become `_`.
/// `Page<User>` → `Page_User_`.
pub fn component_name(raw: &str) -> String {
    raw.chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Build an object JSON Schema from `(key, schema, is_required)` triples.
/// `required` is omitted when empty.
#[cfg(feature = "openapi")]
pub fn object_schema(fields: Vec<(&str, Value, bool)>) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for (key, schema, is_required) in fields {
        if is_required {
            required.push(Value::String(key.to_string()));
        }
        properties.insert(key.to_string(), schema);
    }
    let mut out = Map::new();
    out.insert("type".into(), "object".into());
    if !required.is_empty() {
        out.insert("required".into(), Value::Array(required));
    }
    out.insert("properties".into(), Value::Object(properties));
    Value::Object(out)
}

/// OpenAPI document with the root schema plus every collected nested schema,
/// so `$ref: #/components/schemas/X` targets exist.
#[cfg(feature = "openapi")]
pub fn openapi_document(
    name: &str,
    schema: Value,
    nested: Vec<crate::schema::NestedSchemaEntry>,
) -> Value {
    let mut all: Vec<(&str, Value)> = vec![(name, schema)];
    for (n, f) in nested {
        if n != name {
            all.push((n, f()));
        }
    }
    crate::openapi::to_openapi_document_multi(&all)
}
