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
    match obj.get(key) {
        Some(v) => schema.parse_value(v),
        None => schema
            .parse_missing()
            .map_err(crate::error::absent_field_error),
    }
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

// ---------------------------------------------------------------------------
// `#[derive(Validate)]` on enums (serde representations)
// ---------------------------------------------------------------------------

/// `value` of a unit variant's content (externally / adjacently tagged): absent or `null`.
pub fn expect_unit(content: Option<&Value>) -> Result<(), VldError> {
    match content {
        None | Some(Value::Null) => Ok(()),
        Some(v) => Err(VldError::from_code_with_value(
            crate::error::IssueCode::InvalidType {
                expected: "null".into(),
                received: crate::error::value_type_name(v),
            },
            v,
        )),
    }
}

/// Externally tagged input: `"Variant"` or `{"Variant": content}` (exactly one key).
pub fn external_variant(value: &Value) -> Result<(&str, Option<&Value>), VldError> {
    match value {
        Value::String(s) => Ok((s.as_str(), None)),
        Value::Object(obj) if obj.len() == 1 => {
            let (k, v) = obj.iter().next().expect("one entry");
            Ok((k.as_str(), Some(v)))
        }
        other => Err(VldError::from_code_with_value(
            crate::error::IssueCode::InvalidType {
                expected: "string or single-key object".into(),
                received: crate::error::value_type_name(other),
            },
            other,
        )),
    }
}

/// The string tag of an internally / adjacently tagged enum; errors are at `.tag`.
pub fn enum_tag<'a>(obj: &'a Map<String, Value>, tag: &str) -> Result<&'a str, VldError> {
    let at = |e: VldError| e.with_prefix(PathSegment::Field(tag.to_string()));
    match obj.get(tag) {
        None => Err(at(VldError::from_code(
            crate::error::IssueCode::MissingField,
        ))),
        Some(Value::String(s)) => Ok(s.as_str()),
        Some(other) => Err(at(VldError::from_code_with_value(
            crate::error::IssueCode::InvalidType {
                expected: "string".into(),
                received: crate::error::value_type_name(other),
            },
            other,
        ))),
    }
}

/// No variant has this name: `invalid_discriminator` listing the accepted
/// names, at `.tag` (or the root for externally tagged enums).
pub fn unknown_variant(tag: Option<&str>, name: &str, options: &[&str]) -> VldError {
    let err = VldError::from_code_with_value(
        crate::error::IssueCode::InvalidDiscriminator {
            options: options.iter().map(|o| Value::String((*o).into())).collect(),
        },
        &Value::String(name.into()),
    );
    match tag {
        Some(t) => err.with_prefix(PathSegment::Field(t.to_string())),
        None => err,
    }
}

/// The object without `tag` (content of an internally tagged newtype variant).
pub fn without_key(obj: &Map<String, Value>, tag: &str) -> Value {
    let mut rest = obj.clone();
    rest.remove(tag);
    Value::Object(rest)
}

/// Content of a tuple variant: an array of exactly `len` items.
pub fn expect_tuple(value: &Value, len: usize) -> Result<&[Value], VldError> {
    match value {
        Value::Array(items) if items.len() == len => Ok(items),
        Value::Array(items) => Err(VldError::from_code_with_value(
            crate::error::IssueCode::InvalidLength {
                origin: crate::error::SizeOrigin::Tuple,
                expected: len,
            },
            &Value::from(items.len()),
        )),
        other => Err(VldError::from_code_with_value(
            crate::error::IssueCode::InvalidType {
                expected: "array".into(),
                received: crate::error::value_type_name(other),
            },
            other,
        )),
    }
}

/// Parse tuple item `index`; on failure push its issues (prefixed `[index]`).
pub fn parse_item<S: VldSchema>(
    items: &[Value],
    index: usize,
    schema: &S,
    errors: &mut VldError,
) -> Option<S::Output> {
    match schema.parse_value(&items[index]) {
        Ok(v) => Some(v),
        Err(e) => {
            errors
                .issues
                .extend(e.with_prefix(PathSegment::Index(index)).issues);
            None
        }
    }
}

/// Every variant of an untagged enum failed: `invalid_union` plus each
/// variant's issues (`[variant N]`), like `vld::union`.
pub fn untagged_error(value: &Value, variants: Vec<VldError>) -> VldError {
    crate::combinators::union::union_error(value, variants)
}

/// Schema of one enum variant for [`enum_schema`].
#[cfg(feature = "openapi")]
pub enum VariantSchema {
    /// No content.
    Unit,
    /// Newtype or tuple content (any schema).
    Value(Value),
    /// Struct variant: an object schema.
    Object(Value),
}

/// JSON Schema of a derived enum.
///
/// `tag` / `content` select the serde representation: `(None, None)` external,
/// `(Some, None)` internal, `(Some, Some)` adjacent; `untagged` wins.
#[cfg(feature = "openapi")]
pub fn enum_schema(
    tag: Option<&str>,
    content: Option<&str>,
    untagged: bool,
    deny_unknown: bool,
    variants: Vec<(&str, VariantSchema)>,
) -> Value {
    use serde_json::json;
    let tag_prop = |t: &str, name: &str| json!({ t: { "type": "string", "const": name } });
    let one_of: Vec<Value> = if untagged {
        let any: Vec<Value> = variants
            .into_iter()
            .map(|(_, v)| match v {
                VariantSchema::Unit => json!({ "type": "null" }),
                VariantSchema::Value(s) | VariantSchema::Object(s) => s,
            })
            .collect();
        return json!({ "anyOf": any });
    } else if let Some(t) = tag {
        variants
            .into_iter()
            .map(|(name, v)| match (content, v) {
                (_, VariantSchema::Unit) => {
                    let mut v = json!({
                        "type": "object",
                        "properties": tag_prop(t, name),
                        "required": [t],
                    });
                    if deny_unknown {
                        v["additionalProperties"] = Value::Bool(false);
                    }
                    v
                }
                (Some(c), VariantSchema::Value(s) | VariantSchema::Object(s)) => {
                    let mut props = tag_prop(t, name);
                    props[c] = s;
                    let mut v = json!({ "type": "object", "properties": props, "required": [t, c] });
                    if deny_unknown {
                        v["additionalProperties"] = Value::Bool(false);
                    }
                    v
                }
                (None, VariantSchema::Object(mut s)) => {
                    s["properties"][t] = json!({ "type": "string", "const": name });
                    let required = s
                        .get_mut("required")
                        .and_then(Value::as_array_mut)
                        .map(std::mem::take)
                        .unwrap_or_default();
                    let mut req = vec![Value::String(t.into())];
                    req.extend(required);
                    s["required"] = Value::Array(req);
                    s
                }
                // Internally tagged newtype: the inner object plus the tag.
                (None, VariantSchema::Value(s)) => json!({
                    "allOf": [s, { "type": "object", "properties": tag_prop(t, name), "required": [t] }],
                }),
            })
            .collect()
    } else {
        let mut units = Vec::new();
        let mut out = Vec::new();
        for (name, v) in variants {
            match v {
                VariantSchema::Unit => units.push(Value::String(name.into())),
                VariantSchema::Value(s) | VariantSchema::Object(s) => out.push(json!({
                    "type": "object",
                    "properties": { name: s },
                    "required": [name],
                    "additionalProperties": false,
                })),
            }
        }
        if !units.is_empty() {
            out.insert(0, json!({ "type": "string", "enum": units }));
        }
        out
    };
    let mut schema = json!({ "oneOf": one_of });
    if let Some(t) = tag {
        schema["discriminator"] = json!({ "propertyName": t });
    }
    schema
}

/// Tuple-variant content schema.
#[cfg(feature = "openapi")]
pub fn tuple_schema(items: Vec<Value>) -> Value {
    let n = items.len();
    serde_json::json!({
        "type": "array",
        "prefixItems": items,
        "items": false,
        "minItems": n,
        "maxItems": n,
    })
}

/// Merge the schemas of `#[serde(flatten)]` fields into the struct's object
/// schema: their `properties` / `required` join the object's; anything else
/// (a `$ref`, a `oneOf`) goes into `allOf`.
#[cfg(feature = "openapi")]
pub fn flatten_schema(mut base: Value, parts: Vec<Value>) -> Value {
    for part in parts {
        let mergeable = part.get("properties").is_some_and(Value::is_object)
            && part.get("type").is_none_or(|t| t == "object")
            && part.get("allOf").is_none()
            && part.get("oneOf").is_none()
            && part.get("anyOf").is_none();
        if !mergeable {
            match base.get_mut("allOf").and_then(Value::as_array_mut) {
                Some(all) => all.push(part),
                None => base["allOf"] = Value::Array(vec![part]),
            }
            continue;
        }
        if let Some(props) = part["properties"].as_object() {
            for (k, v) in props {
                base["properties"][k] = v.clone();
            }
        }
        if let Some(req) = part.get("required").and_then(Value::as_array) {
            let mut all = base
                .get("required")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            all.extend(req.iter().cloned());
            base["required"] = Value::Array(all);
        }
    }
    base
}

/// `#[serde(deny_unknown_fields)]`: an `unrecognized_field` issue for every
/// key of `obj` that is not in `known`.
pub fn deny_unknown(obj: &Map<String, Value>, known: &[&str], errors: &mut VldError) {
    for key in obj.keys().filter(|k| !known.contains(&k.as_str())) {
        errors.issues.extend(
            VldError::from_code(crate::error::IssueCode::UnrecognizedField)
                .with_prefix(PathSegment::Field(key.clone()))
                .issues,
        );
    }
}

/// `additionalProperties: false` on an object schema (`deny_unknown_fields`).
#[cfg(feature = "openapi")]
pub fn deny_additional(mut schema: Value) -> Value {
    if let Some(obj) = schema.as_object_mut() {
        obj.insert("additionalProperties".into(), Value::Bool(false));
    }
    schema
}
