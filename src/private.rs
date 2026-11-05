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

/// `(ident, deserialize key, serialize key)` of a `schema!` field.
///
/// `as "key"` (`explicit`) wins; otherwise the field's `#[serde(rename ...)]`,
/// then the struct's `#[serde(rename_all ...)]`, then the bare identifier.
/// `field_attrs` / `struct_attrs` are the `stringify!`d attribute contents
/// (`serde(rename = "x")`); non-serde attributes are ignored.
pub fn serde_keys(
    ident: &'static str,
    explicit: Option<&'static str>,
    field_attrs: &[&str],
    struct_attrs: &[&str],
) -> (&'static str, String, String) {
    let bare = field_key(ident);
    if let Some(k) = explicit {
        return (ident, k.to_string(), k.to_string());
    }
    let (f_de, f_ser) = serde_attr_names(field_attrs, "rename");
    let (s_de, s_ser) = serde_attr_names(struct_attrs, "rename_all");
    let pick = |field: Option<String>, rule: Option<String>| {
        field.unwrap_or_else(|| match rule {
            Some(rule) => rename_field(bare, &rule),
            None => bare.to_string(),
        })
    };
    (ident, pick(f_de, s_de), pick(f_ser, s_ser))
}

/// Split at top-level commas (outside quotes and parentheses).
fn split_top(s: &str) -> Vec<&str> {
    let (mut out, mut depth, mut quoted, mut start) = (vec![], 0i32, false, 0);
    let mut chars = s.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' if quoted => {
                chars.next();
            }
            '"' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => depth -= 1,
            ',' if !quoted && depth == 0 => {
                out.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(s[start..].trim());
    out
}

/// `"x"` → `x`.
fn unquote(s: &str) -> Option<String> {
    s.trim()
        .strip_prefix('"')?
        .strip_suffix('"')
        .map(str::to_string)
}

/// `(deserialize, serialize)` values of `name = "x"` /
/// `name(serialize = "a", deserialize = "b")` in `serde(...)` attribute texts.
fn serde_attr_names(attrs: &[&str], name: &str) -> (Option<String>, Option<String>) {
    let (mut de, mut ser) = (None, None);
    for attr in attrs {
        let Some(inner) = attr
            .trim()
            .strip_prefix("serde")
            .map(str::trim_start)
            .and_then(|a| a.strip_prefix('('))
            .and_then(|a| a.trim_end().strip_suffix(')'))
        else {
            continue;
        };
        for item in split_top(inner) {
            let Some(rest) = item.strip_prefix(name).map(str::trim_start) else {
                continue;
            };
            if let Some(v) = rest.strip_prefix('=').and_then(unquote) {
                de = Some(v.clone());
                ser = Some(v);
            } else if let Some(list) = rest
                .strip_prefix('(')
                .and_then(|l| l.trim_end().strip_suffix(')'))
            {
                for sub in split_top(list) {
                    if let Some((k, v)) = sub.split_once('=') {
                        match k.trim() {
                            "deserialize" => de = unquote(v).or(de),
                            "serialize" => ser = unquote(v).or(ser),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    (de, ser)
}

/// A snake_case field name in a serde `rename_all` convention.
fn rename_field(name: &str, rule: &str) -> String {
    let pascal = || -> String {
        name.split('_')
            .map(|w| {
                let mut c = w.chars();
                c.next()
                    .map(|f| f.to_uppercase().chain(c).collect::<String>())
                    .unwrap_or_default()
            })
            .collect()
    };
    match rule {
        "lowercase" => name.to_lowercase(),
        "UPPERCASE" | "SCREAMING_SNAKE_CASE" => name.to_uppercase(),
        "PascalCase" => pascal(),
        "camelCase" => {
            let p = pascal();
            let mut c = p.chars();
            c.next()
                .map(|f| f.to_lowercase().chain(c).collect())
                .unwrap_or_default()
        }
        "kebab-case" => name.replace('_', "-"),
        "SCREAMING-KEBAB-CASE" => name.replace('_', "-").to_uppercase(),
        _ => name.to_string(),
    }
}

/// Look up a field's key in the table built by [`serde_keys`].
pub fn key_of(
    keys: &'static [(&'static str, String, String)],
    ident: &str,
    ser: bool,
) -> &'static str {
    keys.iter()
        .find(|k| k.0 == ident)
        .map(|k| if ser { k.2.as_str() } else { k.1.as_str() })
        .unwrap_or("")
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
