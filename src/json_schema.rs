//! JSON Schema / OpenAPI generation from `vld` schemas.
//!
//! The [`JsonSchema`] trait is implemented by schema types that can produce a
//! [JSON Schema](https://json-schema.org/) representation. The output is
//! compatible with **OpenAPI 3.1** (which uses JSON Schema directly).
//!
//! Nullable/optional values and unions are emitted as `anyOf` (not `oneOf`):
//! `vld` unions are first-match, and an inner schema that already accepts
//! `null` would make a `oneOf` with `{"type": "null"}` unsatisfiable.
//!
//! # Example
//!
//! ```
//! use vld::prelude::*;
//! use vld::json_schema::JsonSchema;
//!
//! let schema = vld::string().min(2).max(50).email();
//! let js = schema.json_schema();
//! assert_eq!(js["type"], "string");
//! assert_eq!(js["minLength"], 2);
//! assert_eq!(js["format"], "email");
//! ```

use serde_json::{json, Value};

use crate::schema::VldSchema;

pub use crate::schema::NestedSchemaEntry;

/// Trait for types that can produce a JSON Schema representation.
///
/// Implemented for all core `vld` schemas. The output is a `serde_json::Value`
/// representing the JSON Schema object, compatible with OpenAPI 3.1.
pub trait JsonSchema {
    /// Generate a JSON Schema representation.
    fn json_schema(&self) -> Value;

    /// Whether this schema should be included in an object's `required` array.
    ///
    /// By default, fields are required. Modifiers that allow omitting the key
    /// (`.optional()`, `.nullish()`, `.with_default(...)`) return `false`.
    /// `.nullable()` stays required (key must be present; value may be null).
    fn is_required(&self) -> bool {
        true
    }
}

/// Collects [`NestedSchemaEntry`] pairs from nested schemas.
///
/// Used by `schema!` / `#[derive(Validate)]` to discover named nested types
/// (`vld::nested!(T)`) that must be registered as OpenAPI components so the
/// emitted `$ref`s resolve.
pub trait CollectNestedSchemas {
    fn collect_nested_schemas(&self, _out: &mut Vec<NestedSchemaEntry>) {}
}

/// Hint for OpenAPI parameter location when a struct is used with `utoipa::IntoParams`.
///
/// Set via utoipa's `#[into_params(parameter_in = Query)]` attribute on a struct
/// defined with [`schema!`](crate::schema) or `#[derive(Validate)]`.
pub trait OpenApiParameterIn {
    /// Returns `"query"`, `"path"`, `"header"`, or `"cookie"` when the struct is
    /// annotated with `#[into_params(parameter_in = ...)]`, otherwise `None`.
    fn parameter_in() -> Option<&'static str> {
        None
    }
}

/// Wrap a schema so it also accepts `null`: `{"anyOf": [inner, {"type": "null"}]}`.
///
/// Returns `inner` unchanged if it is already such a nullable `anyOf`.
pub fn nullable_schema(inner: Value) -> Value {
    let already_nullable = inner
        .get("anyOf")
        .and_then(Value::as_array)
        .is_some_and(|items| items.iter().any(|i| i.get("type") == Some(&json!("null"))));
    if already_nullable {
        return inner;
    }
    json!({ "anyOf": [inner, { "type": "null" }] })
}

// ---------------------------------------------------------------------------
// Leaf schemas: `to_json_schema()` + no nested schemas
// ---------------------------------------------------------------------------

macro_rules! leaf_schemas {
    ($( $(#[$m:meta])* $t:ty ),* $(,)?) => {$(
        $(#[$m])*
        impl JsonSchema for $t {
            fn json_schema(&self) -> Value {
                self.to_json_schema()
            }
        }
        $(#[$m])*
        impl CollectNestedSchemas for $t {}
    )*};
}

leaf_schemas!(
    crate::primitives::ZString,
    crate::primitives::ZNumber,
    crate::primitives::ZInt,
    crate::primitives::ZBoolean,
    crate::primitives::ZBytes,
    #[cfg(feature = "decimal")]
    crate::primitives::ZDecimal,
    #[cfg(feature = "net")]
    crate::primitives::ZIpNetwork,
    crate::primitives::ZSocketAddr,
    crate::primitives::ZJsonValue,
    #[cfg(feature = "file")]
    crate::primitives::ZFile,
    crate::primitives::ZDuration,
    crate::primitives::ZPath,
    crate::primitives::ZEnum,
    crate::primitives::ZAny,
    #[cfg(any(feature = "chrono", feature = "jiff", feature = "time"))]
    crate::primitives::ZDate,
    #[cfg(any(feature = "chrono", feature = "jiff", feature = "time"))]
    crate::primitives::ZDateTime,
);

// ---------------------------------------------------------------------------
// Wrappers that forward everything to `inner_schema()`
// ---------------------------------------------------------------------------

/// `impl[<generics>] Type, inner = S, where [extra bounds];`
macro_rules! forward_inner {
    ($( impl[$($g:tt)*] $t:ty, inner = $S:ident $(, where [$($w:tt)*])? ; )*) => {$(
        impl<$($g)*> JsonSchema for $t
        where
            $S: VldSchema + JsonSchema,
            $($($w)*)?
        {
            fn json_schema(&self) -> Value {
                self.inner_schema().json_schema()
            }
            fn is_required(&self) -> bool {
                self.inner_schema().is_required()
            }
        }

        impl<$($g)*> CollectNestedSchemas for $t
        where
            $S: VldSchema + CollectNestedSchemas,
            $($($w)*)?
        {
            fn collect_nested_schemas(&self, out: &mut Vec<NestedSchemaEntry>) {
                self.inner_schema().collect_nested_schemas(out);
            }
        }
    )*};
}

forward_inner! {
    impl[S] crate::combinators::ZCatch<S>, inner = S, where [S::Output: Clone];
    impl[S, F] crate::combinators::ZRefine<S, F>, inner = S, where [F: Fn(&S::Output) -> bool];
    impl[S, F, U] crate::combinators::ZTransform<S, F, U>, inner = S, where [F: Fn(S::Output) -> U];
    impl[S] crate::combinators::ZMessage<S>, inner = S;
    impl[S, F] crate::combinators::ZSuperRefine<S, F>, inner = S,
        where [F: Fn(&S::Output, &mut crate::error::VldError)];
    impl[F, S] crate::combinators::ZPreprocess<F, S>, inner = S, where [F: Fn(&Value) -> Value];
}

/// Collect-only forwarding for wrappers whose `JsonSchema` impl is custom.
macro_rules! collect_inner {
    ($( impl[$($g:tt)*] $t:ty, inner = $S:ident $(, where [$($w:tt)*])? ; )*) => {$(
        impl<$($g)*> CollectNestedSchemas for $t
        where
            $S: VldSchema + CollectNestedSchemas,
            $($($w)*)?
        {
            fn collect_nested_schemas(&self, out: &mut Vec<NestedSchemaEntry>) {
                self.inner_schema().collect_nested_schemas(out);
            }
        }
    )*};
}

collect_inner! {
    impl[S] crate::modifiers::ZOptional<S>, inner = S;
    impl[S] crate::modifiers::ZNullable<S>, inner = S;
    impl[S] crate::modifiers::ZNullish<S>, inner = S;
    impl[S] crate::modifiers::ZDefault<S>, inner = S, where [S::Output: Clone];
    impl[S] crate::combinators::ZDescribe<S>, inner = S;
}

// ---------------------------------------------------------------------------
// Modifiers
// ---------------------------------------------------------------------------

impl<S: VldSchema + JsonSchema> JsonSchema for crate::modifiers::ZOptional<S> {
    fn json_schema(&self) -> Value {
        nullable_schema(self.inner_schema().json_schema())
    }
    fn is_required(&self) -> bool {
        false
    }
}

impl<S: VldSchema + JsonSchema> JsonSchema for crate::modifiers::ZNullable<S> {
    fn json_schema(&self) -> Value {
        nullable_schema(self.inner_schema().json_schema())
    }
}

impl<S: VldSchema + JsonSchema> JsonSchema for crate::modifiers::ZNullish<S> {
    fn json_schema(&self) -> Value {
        nullable_schema(self.inner_schema().json_schema())
    }
    fn is_required(&self) -> bool {
        false
    }
}

impl<S: VldSchema + JsonSchema> JsonSchema for crate::modifiers::ZDefault<S>
where
    S::Output: Clone + serde::Serialize,
{
    fn json_schema(&self) -> Value {
        let mut schema = self.inner_schema().json_schema();
        if let (Some(obj), Ok(default)) = (
            schema.as_object_mut(),
            serde_json::to_value(self.default_value()),
        ) {
            obj.insert("default".to_string(), default);
        }
        schema
    }
    fn is_required(&self) -> bool {
        false
    }
}

impl<S: VldSchema + JsonSchema> JsonSchema for crate::combinators::ZDescribe<S> {
    fn json_schema(&self) -> Value {
        let mut schema = self.inner_schema().json_schema();
        let desc = self.description();
        if !desc.is_empty() {
            schema["description"] = Value::String(desc.to_string());
        }
        schema
    }
    fn is_required(&self) -> bool {
        self.inner_schema().is_required()
    }
}

// ---------------------------------------------------------------------------
// Collections
// ---------------------------------------------------------------------------

impl<T: VldSchema + JsonSchema> JsonSchema for crate::collections::ZArray<T> {
    fn json_schema(&self) -> Value {
        self.to_json_schema_inner()
    }
}

impl<T: VldSchema + CollectNestedSchemas> CollectNestedSchemas for crate::collections::ZArray<T> {
    fn collect_nested_schemas(&self, out: &mut Vec<NestedSchemaEntry>) {
        self.element_schema().collect_nested_schemas(out);
    }
}

impl<V: VldSchema + JsonSchema> JsonSchema for crate::collections::ZRecord<V> {
    fn json_schema(&self) -> Value {
        self.to_json_schema_inner()
    }
}

impl<V: VldSchema + CollectNestedSchemas> CollectNestedSchemas for crate::collections::ZRecord<V> {
    fn collect_nested_schemas(&self, out: &mut Vec<NestedSchemaEntry>) {
        self.value_schema_ref().collect_nested_schemas(out);
    }
}

impl<T> JsonSchema for crate::collections::ZSet<T>
where
    T: VldSchema + JsonSchema,
    T::Output: Eq + std::hash::Hash,
{
    fn json_schema(&self) -> Value {
        self.to_json_schema_inner()
    }
}

impl<T> CollectNestedSchemas for crate::collections::ZSet<T>
where
    T: VldSchema + CollectNestedSchemas,
    T::Output: Eq + std::hash::Hash,
{
    fn collect_nested_schemas(&self, out: &mut Vec<NestedSchemaEntry>) {
        self.element_schema().collect_nested_schemas(out);
    }
}

// ZMap parses `[[key, value], ...]`. `ZMap` exposes no key/value schema
// accessors, so the pair items are left unconstrained.
impl<K: VldSchema, V: VldSchema> JsonSchema for crate::collections::ZMap<K, V> {
    fn json_schema(&self) -> Value {
        json!({
            "type": "array",
            "items": { "type": "array", "minItems": 2, "maxItems": 2 }
        })
    }
}

impl<K: VldSchema, V: VldSchema> CollectNestedSchemas for crate::collections::ZMap<K, V> {}

// Tuples parse a fixed-length JSON array.
macro_rules! tuple_schemas {
    ($( $n:literal; $( ($idx:tt, $T:ident) ),+ );* $(;)?) => {$(
        impl<$($T: VldSchema + JsonSchema),+> JsonSchema for ($($T,)+) {
            fn json_schema(&self) -> Value {
                json!({
                    "type": "array",
                    "prefixItems": [ $( self.$idx.json_schema() ),+ ],
                    "items": false,
                    "minItems": $n,
                    "maxItems": $n,
                })
            }
        }

        impl<$($T: VldSchema + CollectNestedSchemas),+> CollectNestedSchemas for ($($T,)+) {
            fn collect_nested_schemas(&self, out: &mut Vec<NestedSchemaEntry>) {
                $( self.$idx.collect_nested_schemas(out); )+
            }
        }
    )*};
}

tuple_schemas! {
    1; (0, A);
    2; (0, A), (1, B);
    3; (0, A), (1, B), (2, C);
    4; (0, A), (1, B), (2, C), (3, D);
    5; (0, A), (1, B), (2, C), (3, D), (4, E);
    6; (0, A), (1, B), (2, C), (3, D), (4, E), (5, F);
}

// ---------------------------------------------------------------------------
// Combinators
// ---------------------------------------------------------------------------

impl<A, B> JsonSchema for crate::combinators::ZUnion2<A, B>
where
    A: VldSchema + JsonSchema,
    B: VldSchema + JsonSchema,
{
    fn json_schema(&self) -> Value {
        json!({ "anyOf": [self.schema_a().json_schema(), self.schema_b().json_schema()] })
    }
}

impl<A, B> CollectNestedSchemas for crate::combinators::ZUnion2<A, B>
where
    A: VldSchema + CollectNestedSchemas,
    B: VldSchema + CollectNestedSchemas,
{
    fn collect_nested_schemas(&self, out: &mut Vec<NestedSchemaEntry>) {
        self.schema_a().collect_nested_schemas(out);
        self.schema_b().collect_nested_schemas(out);
    }
}

impl<A, B, C> JsonSchema for crate::combinators::ZUnion3<A, B, C>
where
    A: VldSchema + JsonSchema,
    B: VldSchema + JsonSchema,
    C: VldSchema + JsonSchema,
{
    fn json_schema(&self) -> Value {
        json!({
            "anyOf": [
                self.schema_a().json_schema(),
                self.schema_b().json_schema(),
                self.schema_c().json_schema(),
            ]
        })
    }
}

impl<A, B, C> CollectNestedSchemas for crate::combinators::ZUnion3<A, B, C>
where
    A: VldSchema + CollectNestedSchemas,
    B: VldSchema + CollectNestedSchemas,
    C: VldSchema + CollectNestedSchemas,
{
    fn collect_nested_schemas(&self, out: &mut Vec<NestedSchemaEntry>) {
        self.schema_a().collect_nested_schemas(out);
        self.schema_b().collect_nested_schemas(out);
        self.schema_c().collect_nested_schemas(out);
    }
}

impl<A, B> JsonSchema for crate::combinators::ZIntersection<A, B>
where
    A: VldSchema + JsonSchema,
    B: VldSchema + JsonSchema,
{
    fn json_schema(&self) -> Value {
        json!({ "allOf": [self.schema_a().json_schema(), self.schema_b().json_schema()] })
    }
}

impl<A, B> CollectNestedSchemas for crate::combinators::ZIntersection<A, B>
where
    A: VldSchema + CollectNestedSchemas,
    B: VldSchema + CollectNestedSchemas,
{
    fn collect_nested_schemas(&self, out: &mut Vec<NestedSchemaEntry>) {
        self.schema_a().collect_nested_schemas(out);
        self.schema_b().collect_nested_schemas(out);
    }
}

// The following types expose no accessors for their inner schema/value, so
// they emit a permissive schema (`{}` accepts anything) rather than failing
// to compile inside `schema!` under `openapi`.

/// `{}` — the literal value is not exposed by `ZLiteral`.
impl<T: crate::primitives::IntoLiteral> JsonSchema for crate::primitives::ZLiteral<T> {
    fn json_schema(&self) -> Value {
        json!({})
    }
}
impl<T: crate::primitives::IntoLiteral> CollectNestedSchemas for crate::primitives::ZLiteral<T> {}

/// `{"type": "object"}` — variants are type-erased and not exposed.
impl JsonSchema for crate::combinators::ZDiscriminatedUnion {
    fn json_schema(&self) -> Value {
        json!({ "type": "object" })
    }
}
impl CollectNestedSchemas for crate::combinators::ZDiscriminatedUnion {}

/// `{}` — a recursive schema cannot be expanded inline.
impl<T: VldSchema, F: Fn() -> T> JsonSchema for crate::combinators::ZLazy<T, F> {
    fn json_schema(&self) -> Value {
        json!({})
    }
}
impl<T: VldSchema, F: Fn() -> T> CollectNestedSchemas for crate::combinators::ZLazy<T, F> {}

/// `{}` — arbitrary validation function.
impl<F, T> JsonSchema for crate::combinators::ZCustom<F, T> {
    fn json_schema(&self) -> Value {
        json!({})
    }
}
impl<F, T> CollectNestedSchemas for crate::combinators::ZCustom<F, T> {}

/// `{}` — the first stage (input schema) is not exposed by `ZPipe`.
impl<A, B> JsonSchema for crate::combinators::ZPipe<A, B>
where
    A: VldSchema,
    B: VldSchema,
    A::Output: serde::Serialize,
{
    fn json_schema(&self) -> Value {
        json!({})
    }
}
impl<A, B> CollectNestedSchemas for crate::combinators::ZPipe<A, B>
where
    A: VldSchema,
    B: VldSchema,
    A::Output: serde::Serialize,
{
}

// ---------------------------------------------------------------------------
// NestedSchema
// ---------------------------------------------------------------------------

impl<T, F> JsonSchema for crate::schema::NestedSchema<T, F>
where
    F: Fn(&Value) -> Result<T, crate::error::VldError>,
{
    fn json_schema(&self) -> Value {
        match self.name {
            Some(name) => json!({ "$ref": format!("#/components/schemas/{}", name) }),
            // Unnamed nested schemas are opaque closures.
            None => json!({ "type": "object" }),
        }
    }
}

impl<T, F> CollectNestedSchemas for crate::schema::NestedSchema<T, F>
where
    F: Fn(&Value) -> Result<T, crate::error::VldError>,
{
    fn collect_nested_schemas(&self, out: &mut Vec<NestedSchemaEntry>) {
        if let (Some(name), Some(f)) = (self.name, self.json_schema_fn) {
            // Dedup by name: skip cycles and diamond references already visited.
            if out.iter().any(|(n, _)| *n == name) {
                return;
            }
            out.push((name, f));
            if let Some(collect) = self.collect_nested_fn {
                collect(out);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Object (dynamic)
// ---------------------------------------------------------------------------

impl JsonSchema for crate::object::ZObject {
    fn json_schema(&self) -> Value {
        self.to_json_schema()
    }
}

impl CollectNestedSchemas for crate::object::ZObject {
    fn collect_nested_schemas(&self, out: &mut Vec<NestedSchemaEntry>) {
        self.collect_nested_into(out);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Wrap a JSON Schema in a minimal OpenAPI 3.1 document structure.
///
/// The schema is placed under `#/components/schemas/{name}`. Nested `$ref`
/// targets are **not** added; types generated by `schema!` /
/// `#[derive(Validate)]` have a `to_openapi_document()` that includes them.
///
/// # Example
///
/// ```
/// use vld::json_schema::{JsonSchema, to_openapi_document};
///
/// let schema = vld::string().email();
/// let doc = to_openapi_document("Email", &schema.json_schema());
/// assert_eq!(doc["openapi"], "3.1.0");
/// ```
pub fn to_openapi_document(name: &str, schema: &Value) -> Value {
    to_openapi_document_multi(&[(name, schema.clone())])
}

/// Wrap multiple schemas in an OpenAPI 3.1 document.
///
/// # Example
///
/// ```
/// use vld::json_schema::{JsonSchema, to_openapi_document_multi};
///
/// let schemas = vec![
///     ("Name", vld::string().min(1).json_schema()),
///     ("Age", vld::number().int().min(0).json_schema()),
/// ];
/// let doc = to_openapi_document_multi(&schemas);
/// assert!(doc["components"]["schemas"]["Name"]["type"] == "string");
/// ```
pub fn to_openapi_document_multi(schemas: &[(&str, Value)]) -> Value {
    let map: serde_json::Map<String, Value> = schemas
        .iter()
        .map(|(name, schema)| (name.to_string(), schema.clone()))
        .collect();
    json!({
        "openapi": "3.1.0",
        "info": { "title": "API", "version": "1.0.0" },
        "paths": {},
        "components": { "schemas": Value::Object(map) }
    })
}
