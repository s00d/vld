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
/// use vld::openapi::JsonSchema as _;
/// use vld_ts::to_zod;
///
/// let schema = vld::string().min(2).email();
/// let ts = to_zod(&schema);
/// assert!(ts.contains("z.string()"));
/// assert!(ts.contains(".email()"));
/// ```
pub fn to_zod<S>(schema: &S) -> String
where
    S: vld::openapi::JsonSchema,
{
    convert(Zod, &schema.json_schema())
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
    S: vld::openapi::JsonSchema,
{
    convert(Valibot, &schema.json_schema())
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
    S: vld::openapi::JsonSchema,
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

    /// Name used for exports (`export const <Name>Schema`).
    ///
    /// Defaults to the type name; `impl_to_zod!(Type, "Name")` overrides it.
    fn zod_name() -> String {
        short_type_name::<Self>()
    }

    /// `export const <Name>Schema = …;` plus `export type <Name> = z.infer<…>;`.
    ///
    /// `$ref`s to nested schemas become `z.lazy(() => <Nested>Schema)`, so export
    /// nested types as well (under their type name).
    fn to_zod_export() -> String {
        zod_export(&Self::zod_name(), &Self::to_zod())
    }
}

/// Trait for types that can generate their Valibot representation.
pub trait ToValibot {
    /// Generate Valibot schema expression for this type (no imports/exports).
    fn to_valibot() -> String;

    /// Name used for exports (`export const <Name>Schema`).
    ///
    /// Defaults to the type name; `impl_to_valibot!(Type, "Name")` overrides it.
    fn valibot_name() -> String {
        short_type_name::<Self>()
    }

    /// `export const <Name>Schema = …;` plus `export type <Name> = v.InferOutput<…>;`.
    fn to_valibot_export() -> String {
        valibot_export(&Self::valibot_name(), &Self::to_valibot())
    }
}

/// Render a named Zod export:
/// `export const <name>Schema = <expr>;\nexport type <name> = z.infer<typeof <name>Schema>;\n`.
///
/// ```
/// let ts = vld_ts::zod_export("Email", &vld_ts::to_zod(&vld::string().email()));
/// assert!(ts.starts_with("export const EmailSchema = z.string().email();"));
/// assert!(ts.contains("export type Email = z.infer<typeof EmailSchema>;"));
/// ```
pub fn zod_export(name: &str, expr: &str) -> String {
    format!(
        "export const {0}Schema = {1};\nexport type {0} = z.infer<typeof {0}Schema>;\n",
        name, expr
    )
}

/// Render a named Valibot export (see [`zod_export`]).
pub fn valibot_export(name: &str, expr: &str) -> String {
    format!(
        "export const {0}Schema = {1};\nexport type {0} = v.InferOutput<typeof {0}Schema>;\n",
        name, expr
    )
}

/// `my::mod::User<T>` → `User`.
fn short_type_name<T: ?Sized>() -> String {
    let full = std::any::type_name::<T>();
    let base = full.split('<').next().unwrap_or(full);
    base.rsplit("::").next().unwrap_or(base).to_string()
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
/// `impl_to_zod!(Type, "Name")` sets the export name used by
/// [`ToZod::to_zod_export`] (default: the type name). Nested `$ref`s always
/// point at `<TypeName>Schema`, so rename only top-level (non-nested) types.
///
/// # Example
///
/// ```
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
/// impl_to_zod!(Address);
/// impl_to_zod!(Order, "OrderBody");
///
/// let ts = Order::to_zod();
/// assert!(ts.starts_with("z.object("));
///
/// let module = format!(
///     "import {{ z }} from \"zod\";\n\n{}\n{}",
///     Address::to_zod_export(),
///     Order::to_zod_export(),
/// );
/// assert!(module.contains("export const AddressSchema = z.object("));
/// assert!(module.contains("export const OrderBodySchema = z.object("));
/// assert!(module.contains("export type OrderBody = z.infer<typeof OrderBodySchema>;"));
/// ```
#[macro_export]
macro_rules! impl_to_zod {
    ($ty:ty $(, $name:expr)?) => {
        impl $crate::ToZod for $ty {
            fn to_zod() -> ::std::string::String {
                $crate::__to_zod_from_value(&<$ty>::json_schema())
            }
            $(
                fn zod_name() -> ::std::string::String {
                    ::std::string::String::from($name)
                }
            )?
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
    // The optional name is accepted for symmetry with `impl_to_zod!`; the JSON
    // Schema itself carries no name.
    ($ty:ty $(, $name:expr)?) => {
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
    ($ty:ty $(, $name:expr)?) => {
        impl $crate::ToValibot for $ty {
            fn to_valibot() -> ::std::string::String {
                $crate::__to_valibot_from_value(&<$ty>::json_schema())
            }
            $(
                fn valibot_name() -> ::std::string::String {
                    ::std::string::String::from($name)
                }
            )?
        }
    };
}

/// Output dialect of the shared JSON Schema walker.
#[derive(Clone, Copy, PartialEq)]
enum Target {
    Zod,
    Valibot,
}

use Target::{Valibot, Zod};

/// `(format, zod action, valibot action)` — the one format table for both targets.
const FORMATS: &[(&str, &str, &str)] = &[
    ("email", "email()", "email()"),
    ("uri", "url()", "url()"),
    ("url", "url()", "url()"),
    ("url-strict", "url()", "url()"),
    ("uuid", "uuid()", "uuid()"),
    ("uuid-v1", "uuid()", "uuid()"),
    ("uuid-v4", "uuid()", "uuid()"),
    ("uuid-v7", "uuid()", "uuid()"),
    ("ipv4", "ip({ version: \"v4\" })", "ipv4()"),
    ("ipv6", "ip({ version: \"v6\" })", "ipv6()"),
    ("ip", "ip()", "ip()"),
    ("date", "date()", "isoDate()"),
    ("date-time", "datetime()", "isoDateTime()"),
    ("time", "time()", "isoTime()"),
    ("country-code", "length(2)", "length(2)"),
    ("currency-code", "length(3)", "length(3)"),
    (
        "phone-e164",
        "regex(/^\\+[1-9]\\d{1,14}$/)",
        "regex(/^\\+[1-9]\\d{1,14}$/)",
    ),
];

impl Target {
    /// Library namespace: `z` or `v`.
    fn ns(self) -> &'static str {
        match self {
            Zod => "z",
            Valibot => "v",
        }
    }

    /// A validation/metadata action call, e.g. `min(2)` (Zod) / `minLength(2)` (Valibot).
    fn act(self, zod: &str, valibot: &str, arg: impl std::fmt::Display) -> String {
        match self {
            Zod => format!("{}({})", zod, arg),
            Valibot => format!("{}({})", valibot, arg),
        }
    }

    /// Attach actions to a base schema: method chain (Zod) or `v.pipe` (Valibot).
    fn pipe(self, base: String, actions: Vec<String>) -> String {
        if actions.is_empty() {
            return base;
        }
        match self {
            Zod => actions.iter().fold(base, |acc, a| format!("{}.{}", acc, a)),
            Valibot => {
                let actions: Vec<String> = actions.iter().map(|a| format!("v.{}", a)).collect();
                format!("v.pipe({}, {})", base, actions.join(", "))
            }
        }
    }

    fn nullable(self, e: String) -> String {
        match self {
            Zod => format!("{}.nullable()", e),
            Valibot => format!("v.nullable({})", e),
        }
    }

    fn optional(self, e: String) -> String {
        match self {
            Zod => format!("{}.optional()", e),
            Valibot => format!("v.optional({})", e),
        }
    }

    /// `default` value: JSON is valid JS expression syntax.
    fn with_default(self, e: String, default: &Value) -> String {
        match self {
            Zod => format!("{}.default({})", e, default),
            Valibot => format!("v.optional({}, {})", e, default),
        }
    }

    fn union(self, mut variants: Vec<String>) -> String {
        if variants.len() == 1 {
            return variants.remove(0);
        }
        format!("{}.union([{}])", self.ns(), variants.join(", "))
    }

    fn literal(self, v: &Value) -> String {
        let ns = self.ns();
        match v {
            Value::Null => format!("{}.null()", ns),
            // JSON scalars are valid (escaped) JS literals.
            Value::String(_) | Value::Number(_) | Value::Bool(_) => {
                format!("{}.literal({})", ns, v)
            }
            _ => format!("{}.unknown()", ns),
        }
    }
}

/// JSON string literal — always a valid JS string literal.
fn js_str(s: &str) -> String {
    Value::String(s.to_string()).to_string()
}

/// Object key: bare if it is a plain JS identifier, quoted otherwise.
fn js_key(k: &str) -> String {
    let mut chars = k.chars();
    let ident = matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '$')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
    if ident {
        k.to_string()
    } else {
        js_str(k)
    }
}

#[doc(hidden)]
pub fn __to_zod_from_value(schema: &Value) -> String {
    convert(Zod, schema)
}

#[doc(hidden)]
pub fn __to_valibot_from_value(schema: &Value) -> String {
    convert(Valibot, schema)
}

/// Shared JSON Schema → Zod/Valibot walker.
///
/// `description` and `default` are handled here for every schema kind, so they
/// survive on unions, refs, enums, booleans etc.
fn convert(t: Target, schema: &Value) -> String {
    let (base, mut actions) = convert_core(t, schema);
    if let Some(desc) = schema.get("description").and_then(Value::as_str) {
        actions.push(t.act("describe", "description", js_str(desc)));
    }
    let e = t.pipe(base, actions);
    match schema.get("default") {
        Some(d) => t.with_default(e, d),
        None => e,
    }
}

/// Base expression plus the actions to attach to it (no description/default).
fn convert_core(t: Target, schema: &Value) -> (String, Vec<String>) {
    let ns = t.ns();

    if let Some(name) = schema
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(extract_ref_name)
    {
        return (format!("{}.lazy(() => {}Schema)", ns, name), vec![]);
    }

    // Core may emit unions/nullables as either `oneOf` or `anyOf`.
    for key in ["oneOf", "anyOf"] {
        if let Some(variants) = schema.get(key).and_then(Value::as_array) {
            let is_null = |v: &Value| v.get("type").and_then(Value::as_str) == Some("null");
            let rest: Vec<String> = variants
                .iter()
                .filter(|v| !is_null(v))
                .map(|v| convert(t, v))
                .collect();
            let has_null = variants.iter().any(is_null);
            return (union_nullable(t, rest, has_null), vec![]);
        }
    }

    if let Some(all_of) = schema.get("allOf").and_then(Value::as_array) {
        let parts: Vec<String> = all_of.iter().map(|v| convert(t, v)).collect();
        let e = match (t, parts.len()) {
            (_, 0) => format!("{}.unknown()", ns),
            (_, 1) => parts[0].clone(),
            (Zod, _) => parts[1..].iter().fold(parts[0].clone(), |acc, p| {
                format!("z.intersection({}, {})", acc, p)
            }),
            (Valibot, _) => format!("v.intersect([{}])", parts.join(", ")),
        };
        return (e, vec![]);
    }

    if let Some(c) = schema.get("const") {
        return (t.literal(c), vec![]);
    }

    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        return (
            t.union(values.iter().map(|v| t.literal(v)).collect()),
            vec![],
        );
    }

    match schema.get("type") {
        Some(Value::String(ty)) => convert_typed(t, ty, schema),
        // `type: ["string", "null"]` → nullable string, `["string", "integer"]` → union
        Some(Value::Array(types)) => {
            let mut has_null = false;
            let mut rest = Vec::new();
            for ty in types.iter().filter_map(Value::as_str) {
                if ty == "null" {
                    has_null = true;
                } else {
                    let (base, actions) = convert_typed(t, ty, schema);
                    rest.push(t.pipe(base, actions));
                }
            }
            (union_nullable(t, rest, has_null), vec![])
        }
        _ => (format!("{}.unknown()", ns), vec![]),
    }
}

fn union_nullable(t: Target, variants: Vec<String>, nullable: bool) -> String {
    match (variants.is_empty(), nullable) {
        (true, true) => format!("{}.null()", t.ns()),
        (true, false) => format!("{}.never()", t.ns()),
        (false, true) => t.nullable(t.union(variants)),
        (false, false) => t.union(variants),
    }
}

fn convert_typed(t: Target, ty: &str, schema: &Value) -> (String, Vec<String>) {
    let ns = t.ns();
    let u64_of = |k: &str| schema.get(k).and_then(Value::as_u64);
    let mut actions = Vec::new();

    match ty {
        "string" => {
            if let Some(n) = u64_of("minLength") {
                actions.push(t.act("min", "minLength", n));
            }
            if let Some(n) = u64_of("maxLength") {
                actions.push(t.act("max", "maxLength", n));
            }
            if let Some(fmt) = schema.get("format").and_then(Value::as_str) {
                if let Some((_, zod, valibot)) = FORMATS.iter().find(|(f, _, _)| *f == fmt) {
                    actions.push(match t {
                        Zod => zod.to_string(),
                        Valibot => valibot.to_string(),
                    });
                }
            }
            if let Some(p) = schema.get("pattern").and_then(Value::as_str) {
                actions.push(format!("regex(new RegExp({}))", js_str(p)));
            }
            (format!("{}.string()", ns), actions)
        }
        "number" | "integer" => {
            if ty == "integer" {
                actions.push(t.act("int", "integer", ""));
            }
            for (key, zod, valibot) in [
                ("minimum", "min", "minValue"),
                ("maximum", "max", "maxValue"),
                ("exclusiveMinimum", "gt", "gtValue"),
                ("exclusiveMaximum", "lt", "ltValue"),
                ("multipleOf", "multipleOf", "multipleOf"),
            ] {
                if let Some(n) = schema.get(key).and_then(Value::as_f64) {
                    actions.push(t.act(zod, valibot, format_number(n)));
                }
            }
            (format!("{}.number()", ns), actions)
        }
        "boolean" => (format!("{}.boolean()", ns), actions),
        "null" => (format!("{}.null()", ns), actions),
        "array" => {
            let items = schema
                .get("items")
                .map(|i| convert(t, i))
                .unwrap_or_else(|| format!("{}.unknown()", ns));
            let mut base = format!("{}.array({})", ns, items);
            if schema.get("uniqueItems").and_then(Value::as_bool) == Some(true) {
                base.push_str(" /* uniqueItems */");
            }
            // Valibot: `minLength`/`maxLength` apply to arrays (`minSize` is for Set/Map/Blob).
            if let Some(n) = u64_of("minItems") {
                actions.push(t.act("min", "minLength", n));
            }
            if let Some(n) = u64_of("maxItems") {
                actions.push(t.act("max", "maxLength", n));
            }
            (base, actions)
        }
        "object" => (convert_object(t, schema), actions),
        _ => (format!("{}.unknown()", ns), actions),
    }
}

fn convert_object(t: Target, schema: &Value) -> String {
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let additional = schema.get("additionalProperties");

    let props = match schema.get("properties").and_then(Value::as_object) {
        Some(p) => p,
        // Record-like / free-form object.
        None => {
            return match (t, additional) {
                (_, Some(a @ Value::Object(_))) => {
                    format!("{0}.record({0}.string(), {1})", t.ns(), convert(t, a))
                }
                (Zod, Some(Value::Bool(false))) => "z.object({}).strict()".to_string(),
                (Valibot, Some(Value::Bool(false))) => "v.strictObject({})".to_string(),
                (Zod, _) => "z.object({}).passthrough()".to_string(),
                (Valibot, _) => "v.objectWithRest({}, v.unknown())".to_string(),
            }
        }
    };

    let fields: Vec<String> = props
        .iter()
        .map(|(key, prop)| {
            let mut e = convert(t, prop);
            // A default already makes the field optional on input.
            if !required.contains(&key.as_str()) && prop.get("default").is_none() {
                e = t.optional(e);
            }
            format!("  {}: {}", js_key(key), e)
        })
        .collect();
    let entries = if fields.is_empty() {
        "{}".to_string()
    } else {
        format!("{{\n{}\n}}", fields.join(",\n"))
    };

    // Unknown keys: `false` → reject, schema → validate as catch-all,
    // otherwise strip (default behaviour of both `z.object` and `v.object`).
    match (t, additional) {
        (Zod, Some(Value::Bool(false))) => format!("z.object({}).strict()", entries),
        (Zod, Some(a @ Value::Object(_))) => {
            format!("z.object({}).catchall({})", entries, convert(t, a))
        }
        (Zod, _) => format!("z.object({})", entries),
        (Valibot, Some(Value::Bool(false))) => format!("v.strictObject({})", entries),
        (Valibot, Some(a @ Value::Object(_))) => {
            format!("v.objectWithRest({}, {})", entries, convert(t, a))
        }
        (Valibot, _) => format!("v.object({})", entries),
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
