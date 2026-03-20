//! # vld-ts — Generate TypeScript Zod schemas from vld
//!
//! `vld-ts` is designed for a **vld-first** workflow:
//! - generate Zod from `vld` schema instances (`to_zod`)
//! - call `Type::to_zod()` on your `vld::schema!` type via `impl_to_zod!(Type)`
//! - generate Valibot from `vld` schema instances (`to_valibot`)
//! - call `Type::to_valibot()` via `impl_to_valibot!(Type)`
//! - generate plain JSON Schema via `to_openapi(...)` or `Type::to_openapi()`
//!
//! # Example
//!
//! ```
//! use vld_ts::{impl_to_openapi, impl_to_valibot, impl_to_zod, to_openapi, to_valibot, to_zod, ToOpenApi, ToRefs, ToValibot, ToZod};
//!
//! vld::schema! {
//!     pub struct Address {
//!         pub city: String => vld::string().min(1),
//!     }
//! }
//!
//! vld::schema! {
//!     pub struct Order {
//!         pub shipping: Address => vld::nested!(Address),
//!     }
//! }
//!
//! impl_to_zod!(Order);
//! impl_to_valibot!(Order);
//! impl_to_openapi!(Order);
//! impl_to_openapi!(Address);
//!
//! let zod = to_zod(&vld::string().min(2).email());
//! assert!(zod.contains(".email()"));
//!
//! let ts = Order::to_zod();
//! assert!(ts.starts_with("z.object("));
//!
//! let valibot = Order::to_valibot();
//! assert!(valibot.starts_with("v.object"));
//!
//! let single_valibot = to_valibot(&vld::string().min(2).email());
//! assert!(single_valibot.contains("v.string()"));
//!
//! let schema_json = Address::to_openapi();
//! assert_eq!(schema_json["type"], "object");
//! let refs = Order::to_refs();
//! assert!(refs.contains(&"#/components/schemas/Address".to_string()));
//!
//! let email_schema = to_openapi(&vld::string().email());
//! assert_eq!(email_schema["type"], "string");
//! ```

use serde_json::Value;

/// Convert a vld schema instance directly to Zod.
///
/// This is the high-level API for users who work with `vld` schemas and do not
/// want to manually call `.json_schema()` and pass raw JSON values around.
///
/// # Example
///
/// ```
/// use vld::json_schema::JsonSchema as _;
/// use vld_ts::to_zod;
///
/// let schema = vld::string().min(2).email();
/// let ts = to_zod(&schema);
/// assert!(ts.contains("z.string()"));
/// assert!(ts.contains(".email()"));
/// ```
pub fn to_zod<S>(schema: &S) -> String
where
    S: vld::json_schema::JsonSchema,
{
    convert_schema(&schema.json_schema())
}

/// Convert a vld schema instance directly to Valibot.
///
/// # Example
///
/// ```
/// use vld_ts::to_valibot;
///
/// let schema = vld::string().min(2).email();
/// let ts = to_valibot(&schema);
/// assert!(ts.contains("v.string()"));
/// assert!(ts.contains("v.email()"));
/// ```
pub fn to_valibot<S>(schema: &S) -> String
where
    S: vld::json_schema::JsonSchema,
{
    convert_schema_valibot(&schema.json_schema())
}

/// Convert a vld schema instance to plain JSON Schema (OpenAPI-compatible).
///
/// # Example
///
/// ```
/// use vld_ts::to_openapi;
///
/// let schema = vld::string().min(2).email();
/// let schema_json = to_openapi(&schema);
/// assert_eq!(schema_json["type"], "string");
/// ```
pub fn to_openapi<S>(schema: &S) -> Value
where
    S: vld::json_schema::JsonSchema,
{
    schema.json_schema()
}

/// Collect all `$ref` paths from a schema object.
///
/// Designed to be used with [`to_openapi`].
///
/// # Example
///
/// ```
/// use vld_ts::openapi_refs;
///
/// let schema = serde_json::json!({
///     "type": "object",
///     "properties": {
///         "address": { "$ref": "#/components/schemas/Address" }
///     }
/// });
///
/// let refs = openapi_refs(&schema);
/// assert_eq!(refs, vec!["#/components/schemas/Address".to_string()]);
/// ```
pub fn openapi_refs(schema: &Value) -> Vec<String> {
    collect_ref_paths(schema)
}

/// Trait for types that can generate their Zod representation.
///
/// Implement with [`impl_to_zod!`], then call `Type::to_zod()`.
pub trait ToZod {
    /// Generate Zod schema expression for this type (no imports/exports).
    fn to_zod() -> String;
}

/// Trait for types that can generate their Valibot representation.
pub trait ToValibot {
    /// Generate Valibot schema expression for this type (no imports/exports).
    fn to_valibot() -> String;
}

/// Trait for types that can generate plain JSON Schema.
pub trait ToOpenApi {
    /// Generate plain JSON Schema for this type.
    fn to_openapi() -> Value;
}

/// Trait for types that can return discovered `$ref` paths.
pub trait ToRefs {
    fn to_refs() -> Vec<String>;
}

/// Implement [`ToZod`] for a `vld::schema!` type.
///
/// # Example
///
/// ```ignore
/// use vld_ts::{impl_to_zod, ToZod};
///
/// vld::schema! {
///     pub struct Address {
///         pub city: String => vld::string().min(1),
///     }
/// }
/// vld::schema! {
///     pub struct Order {
///         pub shipping: Address => vld::nested!(Address),
///     }
/// }
///
/// impl_to_zod!(Order);
///
/// let ts = Order::to_zod();
/// assert!(ts.starts_with("z.object("));
/// ```
#[macro_export]
macro_rules! impl_to_zod {
    ($ty:ty) => {
        impl $crate::ToZod for $ty {
            fn to_zod() -> ::std::string::String {
                $crate::__to_zod_from_value(&<$ty>::json_schema())
            }
        }
    };
    ($ty:ty, $name:expr) => {
        impl $crate::ToZod for $ty {
            fn to_zod() -> ::std::string::String {
                let _ = $name;
                $crate::__to_zod_from_value(&<$ty>::json_schema())
            }
        }
    };
}

/// Implement [`ToOpenApi`] for a `vld::schema!` type.
///
/// # Example
///
/// ```ignore
/// use vld_ts::{impl_to_openapi, ToOpenApi};
///
/// vld::schema! {
///     pub struct User {
///         pub email: String => vld::string().email(),
///     }
/// }
///
/// impl_to_openapi!(User);
/// let schema_json = User::to_openapi();
/// assert_eq!(schema_json["type"], "object");
/// ```
#[macro_export]
macro_rules! impl_to_openapi {
    ($ty:ty) => {
        impl $crate::ToOpenApi for $ty {
            fn to_openapi() -> ::serde_json::Value {
                <$ty>::json_schema()
            }
        }
        impl $crate::ToRefs for $ty {
            fn to_refs() -> ::std::vec::Vec<::std::string::String> {
                $crate::openapi_refs(&<$ty>::json_schema())
            }
        }
    };
    ($ty:ty, $name:expr) => {
        impl $crate::ToOpenApi for $ty {
            fn to_openapi() -> ::serde_json::Value {
                let _ = $name;
                <$ty>::json_schema()
            }
        }
        impl $crate::ToRefs for $ty {
            fn to_refs() -> ::std::vec::Vec<::std::string::String> {
                let _ = $name;
                $crate::openapi_refs(&<$ty>::json_schema())
            }
        }
    };
}

/// Implement [`ToValibot`] for a `vld::schema!` type.
///
/// # Example
///
/// ```ignore
/// use vld_ts::{impl_to_valibot, ToValibot};
///
/// vld::schema! {
///     pub struct User {
///         pub email: String => vld::string().email(),
///     }
/// }
///
/// impl_to_valibot!(User);
/// let ts = User::to_valibot();
/// assert!(ts.starts_with("v.object"));
/// ```
#[macro_export]
macro_rules! impl_to_valibot {
    ($ty:ty) => {
        impl $crate::ToValibot for $ty {
            fn to_valibot() -> ::std::string::String {
                $crate::__to_valibot_from_value(&<$ty>::json_schema())
            }
        }
    };
    ($ty:ty, $name:expr) => {
        impl $crate::ToValibot for $ty {
            fn to_valibot() -> ::std::string::String {
                let _ = $name;
                $crate::__to_valibot_from_value(&<$ty>::json_schema())
            }
        }
    };
}

#[doc(hidden)]
pub fn __to_zod_from_value(schema: &Value) -> String {
    convert_schema(schema)
}

#[doc(hidden)]
pub fn __to_valibot_from_value(schema: &Value) -> String {
    convert_schema_valibot(schema)
}

fn convert_schema(schema: &Value) -> String {
    // Handle references to named schemas.
    if let Some(ref_path) = schema.get("$ref").and_then(|v| v.as_str()) {
        if let Some(name) = extract_ref_name(ref_path) {
            return format!("z.lazy(() => {}Schema)", name);
        }
    }

    // Handle combinators first
    if let Some(one_of) = schema.get("oneOf").and_then(|v| v.as_array()) {
        // Check if it's a nullable pattern: [inner, {"type": "null"}]
        if one_of.len() == 2 {
            let is_null_0 = one_of[0].get("type").and_then(|t| t.as_str()) == Some("null");
            let is_null_1 = one_of[1].get("type").and_then(|t| t.as_str()) == Some("null");
            if is_null_1 && !is_null_0 {
                return format!("{}.nullable()", convert_schema(&one_of[0]));
            }
            if is_null_0 && !is_null_1 {
                return format!("{}.nullable()", convert_schema(&one_of[1]));
            }
        }
        let variants: Vec<String> = one_of.iter().map(convert_schema).collect();
        return format!("z.union([{}])", variants.join(", "));
    }

    if let Some(all_of) = schema.get("allOf").and_then(|v| v.as_array()) {
        let parts: Vec<String> = all_of.iter().map(convert_schema).collect();
        if parts.len() == 1 {
            return parts[0].clone();
        }
        let mut result = parts[0].clone();
        for p in &parts[1..] {
            result = format!("z.intersection({}, {})", result, p);
        }
        return result;
    }

    if let Some(any_of) = schema.get("anyOf").and_then(|v| v.as_array()) {
        let variants: Vec<String> = any_of.iter().map(convert_schema).collect();
        return format!("z.union([{}])", variants.join(", "));
    }

    // Handle enum
    if let Some(enum_vals) = schema.get("enum").and_then(|v| v.as_array()) {
        let literals: Vec<String> = enum_vals
            .iter()
            .map(|v| match v {
                Value::String(s) => format!("z.literal(\"{}\")", s),
                Value::Number(n) => format!("z.literal({})", n),
                Value::Bool(b) => format!("z.literal({})", b),
                Value::Null => "z.null()".to_string(),
                _ => "z.unknown()".to_string(),
            })
            .collect();
        if literals.len() == 1 {
            return literals[0].clone();
        }
        return format!("z.union([{}])", literals.join(", "));
    }

    // Get the type
    let type_str = schema.get("type").and_then(|t| t.as_str()).unwrap_or("");

    match type_str {
        "string" => convert_string(schema),
        "number" => convert_number(schema),
        "integer" => convert_integer(schema),
        "boolean" => "z.boolean()".to_string(),
        "null" => "z.null()".to_string(),
        "array" => convert_array(schema),
        "object" => convert_object(schema),
        _ => "z.unknown()".to_string(),
    }
}

fn convert_string(schema: &Value) -> String {
    let mut s = "z.string()".to_string();

    if let Some(min) = schema.get("minLength").and_then(|v| v.as_u64()) {
        s.push_str(&format!(".min({})", min));
    }
    if let Some(max) = schema.get("maxLength").and_then(|v| v.as_u64()) {
        s.push_str(&format!(".max({})", max));
    }
    if let Some(format) = schema.get("format").and_then(|v| v.as_str()) {
        match format {
            "email" => s.push_str(".email()"),
            "uri" | "url" => s.push_str(".url()"),
            "uuid" => s.push_str(".uuid()"),
            "ipv4" => s.push_str(".ip({ version: \"v4\" })"),
            "ipv6" => s.push_str(".ip({ version: \"v6\" })"),
            "date" => s.push_str(".date()"),
            "date-time" => s.push_str(".datetime()"),
            "time" => s.push_str(".time()"),
            _ => { /* skip unknown formats */ }
        }
    }
    if let Some(pattern) = schema.get("pattern").and_then(|v| v.as_str()) {
        s.push_str(&format!(".regex(/{}/)", pattern));
    }

    add_description(&mut s, schema);
    s
}

fn convert_number(schema: &Value) -> String {
    let mut s = "z.number()".to_string();
    add_numeric_constraints(&mut s, schema);
    add_description(&mut s, schema);
    s
}

fn convert_integer(schema: &Value) -> String {
    let mut s = "z.number().int()".to_string();
    add_numeric_constraints(&mut s, schema);
    add_description(&mut s, schema);
    s
}

fn add_numeric_constraints(s: &mut String, schema: &Value) {
    if let Some(min) = schema.get("minimum").and_then(|v| v.as_f64()) {
        s.push_str(&format!(".min({})", format_number(min)));
    }
    if let Some(max) = schema.get("maximum").and_then(|v| v.as_f64()) {
        s.push_str(&format!(".max({})", format_number(max)));
    }
    if let Some(gt) = schema.get("exclusiveMinimum").and_then(|v| v.as_f64()) {
        s.push_str(&format!(".gt({})", format_number(gt)));
    }
    if let Some(lt) = schema.get("exclusiveMaximum").and_then(|v| v.as_f64()) {
        s.push_str(&format!(".lt({})", format_number(lt)));
    }
    if let Some(mul) = schema.get("multipleOf").and_then(|v| v.as_f64()) {
        s.push_str(&format!(".multipleOf({})", format_number(mul)));
    }
}

fn convert_array(schema: &Value) -> String {
    let items = schema
        .get("items")
        .map(convert_schema)
        .unwrap_or_else(|| "z.unknown()".to_string());

    let mut s = format!("z.array({})", items);

    if let Some(min) = schema.get("minItems").and_then(|v| v.as_u64()) {
        s.push_str(&format!(".min({})", min));
    }
    if let Some(max) = schema.get("maxItems").and_then(|v| v.as_u64()) {
        s.push_str(&format!(".max({})", max));
    }
    if schema.get("uniqueItems").and_then(|v| v.as_bool()) == Some(true) {
        s.push_str(" /* uniqueItems */");
    }

    add_description(&mut s, schema);
    s
}

fn convert_object(schema: &Value) -> String {
    let required: Vec<&str> = schema
        .get("required")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let props = schema.get("properties").and_then(|v| v.as_object());

    if let Some(props) = props {
        let mut fields: Vec<String> = Vec::new();
        for (key, prop_schema) in props {
            let mut zod = convert_schema(prop_schema);
            if !required.contains(&key.as_str()) {
                zod.push_str(".optional()");
            }
            fields.push(format!("  {}: {}", key, zod));
        }

        let mut s = format!("z.object({{\n{}\n}})", fields.join(",\n"));

        // additionalProperties
        if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
            s.push_str(".strict()");
        }

        add_description(&mut s, schema);
        s
    } else {
        // Record-like schema
        if let Some(additional) = schema.get("additionalProperties") {
            if additional.is_object() {
                let value_schema = convert_schema(additional);
                return format!("z.record(z.string(), {})", value_schema);
            }
        }

        let mut s = "z.object({})".to_string();
        if schema.get("additionalProperties") != Some(&Value::Bool(false)) {
            s.push_str(".passthrough()");
        }
        s
    }
}

fn add_description(s: &mut String, schema: &Value) {
    if let Some(desc) = schema.get("description").and_then(|v| v.as_str()) {
        s.push_str(&format!(".describe(\"{}\")", desc.replace('"', "\\\"")));
    }
}

fn format_number(n: f64) -> String {
    if n == n.floor() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}

fn extract_ref_name(ref_path: &str) -> Option<&str> {
    ref_path
        .strip_prefix("#/components/schemas/")
        .or_else(|| ref_path.strip_prefix("#/$defs/"))
        .or_else(|| ref_path.strip_prefix("#/definitions/"))
}

fn collect_ref_paths(schema: &Value) -> Vec<String> {
    let mut out = std::collections::BTreeSet::new();
    collect_ref_paths_rec(schema, &mut out);
    out.into_iter().collect()
}

fn collect_ref_paths_rec(value: &Value, out: &mut std::collections::BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(ref_path)) = map.get("$ref") {
                out.insert(ref_path.clone());
            }
            for v in map.values() {
                collect_ref_paths_rec(v, out);
            }
        }
        Value::Array(items) => {
            for v in items {
                collect_ref_paths_rec(v, out);
            }
        }
        _ => {}
    }
}

fn convert_schema_valibot(schema: &Value) -> String {
    if let Some(ref_path) = schema.get("$ref").and_then(|v| v.as_str()) {
        if let Some(name) = extract_ref_name(ref_path) {
            return format!("v.lazy(() => {}Schema)", name);
        }
    }

    if let Some(one_of) = schema.get("oneOf").and_then(|v| v.as_array()) {
        if one_of.len() == 2 {
            let is_null_0 = one_of[0].get("type").and_then(|t| t.as_str()) == Some("null");
            let is_null_1 = one_of[1].get("type").and_then(|t| t.as_str()) == Some("null");
            if is_null_1 && !is_null_0 {
                return format!("v.nullable({})", convert_schema_valibot(&one_of[0]));
            }
            if is_null_0 && !is_null_1 {
                return format!("v.nullable({})", convert_schema_valibot(&one_of[1]));
            }
        }
        let variants: Vec<String> = one_of.iter().map(convert_schema_valibot).collect();
        return format!("v.union([{}])", variants.join(", "));
    }

    if let Some(all_of) = schema.get("allOf").and_then(|v| v.as_array()) {
        let parts: Vec<String> = all_of.iter().map(convert_schema_valibot).collect();
        if parts.len() == 1 {
            return parts[0].clone();
        }
        return format!("v.intersect([{}])", parts.join(", "));
    }

    if let Some(any_of) = schema.get("anyOf").and_then(|v| v.as_array()) {
        let variants: Vec<String> = any_of.iter().map(convert_schema_valibot).collect();
        return format!("v.union([{}])", variants.join(", "));
    }

    if let Some(enum_vals) = schema.get("enum").and_then(|v| v.as_array()) {
        let literals: Vec<String> = enum_vals
            .iter()
            .map(|v| match v {
                Value::String(s) => format!("v.literal(\"{}\")", escape_js_string(s)),
                Value::Number(n) => format!("v.literal({})", n),
                Value::Bool(b) => format!("v.literal({})", b),
                Value::Null => "v.null()".to_string(),
                _ => "v.unknown()".to_string(),
            })
            .collect();
        if literals.len() == 1 {
            return literals[0].clone();
        }
        return format!("v.union([{}])", literals.join(", "));
    }

    let type_str = schema.get("type").and_then(|t| t.as_str()).unwrap_or("");
    match type_str {
        "string" => convert_string_valibot(schema),
        "number" => convert_number_valibot(schema),
        "integer" => convert_integer_valibot(schema),
        "boolean" => wrap_description_valibot("v.boolean()".to_string(), schema),
        "null" => wrap_description_valibot("v.null()".to_string(), schema),
        "array" => convert_array_valibot(schema),
        "object" => convert_object_valibot(schema),
        _ => "v.unknown()".to_string(),
    }
}

fn convert_string_valibot(schema: &Value) -> String {
    let mut actions = Vec::new();
    if let Some(min) = schema.get("minLength").and_then(|v| v.as_u64()) {
        actions.push(format!("v.minLength({})", min));
    }
    if let Some(max) = schema.get("maxLength").and_then(|v| v.as_u64()) {
        actions.push(format!("v.maxLength({})", max));
    }
    if let Some(format) = schema.get("format").and_then(|v| v.as_str()) {
        match format {
            "email" => actions.push("v.email()".to_string()),
            "uri" | "url" => actions.push("v.url()".to_string()),
            "uuid" => actions.push("v.uuid()".to_string()),
            "ipv4" => actions.push("v.ipv4()".to_string()),
            "ipv6" => actions.push("v.ipv6()".to_string()),
            "date" => actions.push("v.isoDate()".to_string()),
            "date-time" => actions.push("v.isoDateTime()".to_string()),
            "time" => actions.push("v.isoTime()".to_string()),
            _ => {}
        }
    }
    if let Some(pattern) = schema.get("pattern").and_then(|v| v.as_str()) {
        actions.push(format!(
            "v.regex(new RegExp(\"{}\"))",
            escape_js_string(pattern)
        ));
    }
    if let Some(desc) = schema.get("description").and_then(|v| v.as_str()) {
        actions.push(format!("v.description(\"{}\")", escape_js_string(desc)));
    }
    wrap_pipe_valibot("v.string()".to_string(), actions)
}

fn convert_number_valibot(schema: &Value) -> String {
    let mut actions = numeric_actions_valibot(schema);
    if let Some(desc) = schema.get("description").and_then(|v| v.as_str()) {
        actions.push(format!("v.description(\"{}\")", escape_js_string(desc)));
    }
    wrap_pipe_valibot("v.number()".to_string(), actions)
}

fn convert_integer_valibot(schema: &Value) -> String {
    let mut actions = vec!["v.integer()".to_string()];
    actions.extend(numeric_actions_valibot(schema));
    if let Some(desc) = schema.get("description").and_then(|v| v.as_str()) {
        actions.push(format!("v.description(\"{}\")", escape_js_string(desc)));
    }
    wrap_pipe_valibot("v.number()".to_string(), actions)
}

fn numeric_actions_valibot(schema: &Value) -> Vec<String> {
    let mut actions = Vec::new();
    if let Some(min) = schema.get("minimum").and_then(|v| v.as_f64()) {
        actions.push(format!("v.minValue({})", format_number(min)));
    }
    if let Some(max) = schema.get("maximum").and_then(|v| v.as_f64()) {
        actions.push(format!("v.maxValue({})", format_number(max)));
    }
    if let Some(gt) = schema.get("exclusiveMinimum").and_then(|v| v.as_f64()) {
        actions.push(format!("v.gtValue({})", format_number(gt)));
    }
    if let Some(lt) = schema.get("exclusiveMaximum").and_then(|v| v.as_f64()) {
        actions.push(format!("v.ltValue({})", format_number(lt)));
    }
    if let Some(mul) = schema.get("multipleOf").and_then(|v| v.as_f64()) {
        actions.push(format!("v.multipleOf({})", format_number(mul)));
    }
    actions
}

fn convert_array_valibot(schema: &Value) -> String {
    let items = schema
        .get("items")
        .map(convert_schema_valibot)
        .unwrap_or_else(|| "v.unknown()".to_string());
    let base = format!("v.array({})", items);
    let mut actions = Vec::new();
    if let Some(min) = schema.get("minItems").and_then(|v| v.as_u64()) {
        actions.push(format!("v.minSize({})", min));
    }
    if let Some(max) = schema.get("maxItems").and_then(|v| v.as_u64()) {
        actions.push(format!("v.maxSize({})", max));
    }
    if let Some(desc) = schema.get("description").and_then(|v| v.as_str()) {
        actions.push(format!("v.description(\"{}\")", escape_js_string(desc)));
    }
    if schema.get("uniqueItems").and_then(|v| v.as_bool()) == Some(true) {
        actions.push("/* uniqueItems */".to_string());
    }
    wrap_pipe_valibot(base, actions)
}

fn convert_object_valibot(schema: &Value) -> String {
    let required: Vec<&str> = schema
        .get("required")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let props = schema.get("properties").and_then(|v| v.as_object());
    let additional = schema.get("additionalProperties");

    let mut base = if let Some(props) = props {
        let mut fields: Vec<String> = Vec::new();
        for (key, prop_schema) in props {
            let mut inner = convert_schema_valibot(prop_schema);
            if !required.contains(&key.as_str()) {
                inner = format!("v.optional({})", inner);
            }
            fields.push(format!("  {}: {}", key, inner));
        }
        let entries = format!("{{\n{}\n}}", fields.join(",\n"));
        match additional {
            Some(Value::Bool(false)) => format!("v.strictObject({})", entries),
            Some(Value::Object(_)) => format!(
                "v.objectWithRest({}, {})",
                entries,
                convert_schema_valibot(additional.unwrap_or(&Value::Null))
            ),
            _ => format!("v.objectWithRest({}, v.unknown())", entries),
        }
    } else {
        match additional {
            Some(Value::Object(_)) => format!(
                "v.record(v.string(), {})",
                convert_schema_valibot(additional.unwrap_or(&Value::Null))
            ),
            Some(Value::Bool(false)) => "v.strictObject({})".to_string(),
            _ => "v.objectWithRest({}, v.unknown())".to_string(),
        }
    };

    base = wrap_description_valibot(base, schema);
    base
}

fn wrap_pipe_valibot(base: String, actions: Vec<String>) -> String {
    if actions.is_empty() {
        base
    } else {
        format!("v.pipe({}, {})", base, actions.join(", "))
    }
}

fn wrap_description_valibot(base: String, schema: &Value) -> String {
    if let Some(desc) = schema.get("description").and_then(|v| v.as_str()) {
        return format!(
            "v.pipe({}, v.description(\"{}\"))",
            base,
            escape_js_string(desc)
        );
    }
    base
}

fn escape_js_string(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
