//! [`JsonSchema`] / [`CollectNestedSchemas`] impls for the combinators.

use serde_json::{json, Value};

use crate::openapi::{CollectNestedSchemas, JsonSchema};
use crate::schema::{NestedSchemaEntry, VldSchema};

crate::openapi::forward_inner_schema! {
    impl[S, F] crate::combinators::ZRefine<S, F>, inner = S, where [F: Fn(&S::Output) -> bool];
    impl[S, F, U] crate::combinators::ZTransform<S, F, U>, inner = S, where [F: Fn(S::Output) -> U];
    impl[S] crate::combinators::ZMessage<S>, inner = S;
    impl[S, F] crate::combinators::ZSuperRefine<S, F>, inner = S,
        where [F: Fn(&S::Output, &mut crate::error::VldError)];
    impl[F, S] crate::combinators::ZPreprocess<F, S>, inner = S, where [F: Fn(&Value) -> Value];
}

crate::openapi::collect_inner_schemas! {
    impl[S] crate::combinators::ZDescribe<S>, inner = S;
    impl[S] crate::combinators::ZCatch<S>, inner = S, where [S::Output: Clone];
}

/// The inner schema; never `required` (a missing key yields the fallback).
impl<S: VldSchema + JsonSchema> JsonSchema for crate::combinators::ZCatch<S>
where
    S::Output: Clone,
{
    fn json_schema(&self) -> Value {
        self.inner_schema().json_schema()
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

/// `{"anyOf": [<variant schemas>]}`.
impl JsonSchema for crate::combinators::ZDiscriminatedUnion {
    fn json_schema(&self) -> Value {
        let variants: Vec<Value> = self.variants().map(|(_, s)| s.dyn_json_schema()).collect();
        json!({ "anyOf": variants })
    }
}

impl CollectNestedSchemas for crate::combinators::ZDiscriminatedUnion {
    fn collect_nested_schemas(&self, out: &mut Vec<NestedSchemaEntry>) {
        for (_, s) in self.variants() {
            s.dyn_collect_nested_schemas(out);
        }
    }
}

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

/// `{}` — the output of the first stage is re-validated by the second,
/// so no single input schema describes the pipe.
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
