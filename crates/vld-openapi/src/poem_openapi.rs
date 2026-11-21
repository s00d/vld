//! poem-openapi 5: `vld` types as poem-openapi request bodies and schemas.
//!
//! - [`impl_type!`] implements poem-openapi's `Type` and `ParseFromJSON` for a
//!   `vld::schema!` / `#[derive(Validate)]` type: its OpenAPI schema is the
//!   vld `json_schema()` (a named component, nested `vld::nested!` types
//!   included), and parsing validates with vld. `Json<User>` then works in
//!   `#[oai]` operations; a failure is poem-openapi's own **400** with vld's
//!   message.
//! - [`VldJson<T>`] is a request payload for such a type that answers
//!   **422** with every issue (`{"error": "Validation failed", "issues":
//!   [{"path", "message", "code"}]}`, the body `vld_web` sends) instead of
//!   one 400 text line. A wrong `Content-Type` stays poem-openapi's 415. As a
//!   response, `VldJson<T>` serializes `T` (`T: serde::Serialize`).
//! - [`json_schema_to_meta`] converts any vld JSON Schema to a `MetaSchemaRef`.
//!
//! The schema is converted to the OpenAPI 3.0 dialect
//! ([`crate::validator::to_openapi3`]) first. Keywords poem-openapi's
//! `MetaSchema` has no field for (`not`, `contains`, `propertyNames`, `x-*`,
//! `additionalProperties: false`) are dropped from the document; validation
//! still applies them.
//!
//! ```rust
//! use poem_openapi::{OpenApi, payload::Json};
//! use vld_openapi::poem_openapi::{VldJson, impl_type};
//!
//! vld::schema! {
//!     #[derive(Debug, serde::Serialize)]
//!     pub struct NewUser {
//!         pub name: String => vld::string().min(2),
//!         pub age: i64 => vld::number().int().min(18),
//!     }
//! }
//! impl_type!(NewUser);
//!
//! struct Api;
//!
//! #[OpenApi]
//! impl Api {
//!     /// 422 with every issue on invalid input.
//!     #[oai(path = "/users", method = "post")]
//!     async fn create(&self, body: VldJson<NewUser>) -> Json<String> {
//!         Json(body.0.name)
//!     }
//!
//!     /// poem-openapi's 400 on invalid input.
//!     #[oai(path = "/users/plain", method = "post")]
//!     async fn create_plain(&self, body: Json<NewUser>) -> Json<i64> {
//!         Json(body.0.age)
//!     }
//! }
//!
//! let service = poem_openapi::OpenApiService::new(Api, "Users", "1.0");
//! assert!(service.spec().contains("\"minLength\": 2"));
//! ```
//!
//! Guide: <https://s00d.github.io/vld/integrations/openapi/poem-openapi>

pub use ::poem_openapi;

use ::poem_openapi::ApiResponse;
use ::poem_openapi::payload::{ParsePayload, Payload};
use ::poem_openapi::registry::{
    MetaMediaType, MetaResponse, MetaResponses, MetaSchema, MetaSchemaRef, Registry,
};
use ::poem_openapi::types::Type;
use poem::{FromRequest, IntoResponse, Request, RequestBody, Response};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use std::sync::Mutex;
use vld::error::{IssueCode, VldError};
use vld::schema::VldParse;

/// Convert a vld JSON Schema to a poem-openapi schema (OpenAPI 3.0).
///
/// `{"$ref": "#/components/schemas/X"}` becomes a reference to `X`.
/// Keywords `MetaSchema` cannot hold are dropped (see the [module docs](self)).
pub fn json_schema_to_meta(value: &Value) -> MetaSchemaRef {
    let mut value = value.clone();
    crate::validator::to_openapi3(&mut value);
    meta_ref(&value)
}

fn meta_ref(value: &Value) -> MetaSchemaRef {
    let Value::Object(map) = value else {
        return MetaSchemaRef::Inline(Box::new(MetaSchema::ANY));
    };
    if let Some(name) = map
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|r| r.strip_prefix("#/components/schemas/"))
    {
        return MetaSchemaRef::Reference(name.to_string());
    }
    MetaSchemaRef::Inline(Box::new(meta_schema(map)))
}

fn meta_schema(map: &Map<String, Value>) -> MetaSchema {
    let str_of = |k: &str| map.get(k).and_then(Value::as_str);
    let f64_of = |k: &str| map.get(k).and_then(Value::as_f64);
    let usize_of = |k: &str| map.get(k).and_then(Value::as_u64).map(|n| n as usize);
    let bool_of = |k: &str| map.get(k).and_then(Value::as_bool);
    let list = |k: &str| -> Vec<MetaSchemaRef> {
        map.get(k)
            .and_then(Value::as_array)
            .map(|a| a.iter().map(meta_ref).collect())
            .unwrap_or_default()
    };
    let ty = match map.get("type") {
        Some(Value::String(t)) => intern(t),
        // OpenAPI 3.0 has one `type`: a multi-type schema stays untyped.
        _ => "",
    };
    MetaSchema {
        ty,
        format: str_of("format").map(intern),
        title: str_of("title").map(str::to_owned),
        description: str_of("description").map(intern),
        default: map.get("default").cloned(),
        required: map
            .get("required")
            .and_then(Value::as_array)
            .map(|r| r.iter().filter_map(Value::as_str).map(intern).collect())
            .unwrap_or_default(),
        properties: map
            .get("properties")
            .and_then(Value::as_object)
            .map(|p| p.iter().map(|(k, v)| (intern(k), meta_ref(v))).collect())
            .unwrap_or_default(),
        items: map.get("items").map(|i| Box::new(meta_ref(i))),
        additional_properties: map
            .get("additionalProperties")
            .filter(|v| v.is_object())
            .map(|v| Box::new(meta_ref(v))),
        enum_items: match (map.get("enum"), map.get("const")) {
            (Some(Value::Array(v)), _) => v.clone(),
            (_, Some(c)) => vec![c.clone()],
            _ => Vec::new(),
        },
        nullable: bool_of("nullable") == Some(true),
        any_of: list("anyOf"),
        one_of: list("oneOf"),
        all_of: list("allOf"),
        read_only: bool_of("readOnly") == Some(true),
        write_only: bool_of("writeOnly") == Some(true),
        deprecated: bool_of("deprecated") == Some(true),
        example: map.get("example").cloned(),
        multiple_of: f64_of("multipleOf"),
        maximum: f64_of("maximum"),
        exclusive_maximum: bool_of("exclusiveMaximum"),
        minimum: f64_of("minimum"),
        exclusive_minimum: bool_of("exclusiveMinimum"),
        max_length: usize_of("maxLength"),
        min_length: usize_of("minLength"),
        pattern: str_of("pattern").map(str::to_owned),
        max_items: usize_of("maxItems"),
        min_items: usize_of("minItems"),
        unique_items: bool_of("uniqueItems"),
        max_properties: usize_of("maxProperties"),
        min_properties: usize_of("minProperties"),
        ..MetaSchema::ANY
    }
}

/// `MetaSchema` holds `&'static str` names; each distinct text is leaked once.
fn intern(s: &str) -> &'static str {
    // ponytail: one global set; distinct names of the documented types only.
    static POOL: Mutex<BTreeSet<&'static str>> = Mutex::new(BTreeSet::new());
    let mut pool = POOL.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = pool.get(s) {
        return s;
    }
    let s: &'static str = Box::leak(s.to_owned().into_boxed_str());
    pool.insert(s);
    s
}

/// Register `T` under `name` and its nested components. Used by [`impl_type!`].
#[doc(hidden)]
pub fn __register<T>(
    registry: &mut Registry,
    name: &str,
    schema: fn() -> Value,
    nested: Vec<vld::openapi::NestedSchemaEntry>,
) {
    registry.create_schema::<T, _>(name.to_string(), |_| match json_schema_to_meta(&schema()) {
        MetaSchemaRef::Inline(s) => *s,
        r @ MetaSchemaRef::Reference(_) => MetaSchema {
            all_of: vec![r],
            ..MetaSchema::ANY
        },
    });
    for (nested_name, schema) in nested {
        if !registry.schemas.contains_key(nested_name) {
            let MetaSchemaRef::Inline(s) = json_schema_to_meta(&schema()) else {
                continue;
            };
            registry.schemas.insert(nested_name.to_string(), *s);
        }
    }
}

/// vld parse for [`impl_type!`]'s `ParseFromJSON`: a missing value is `null`.
#[doc(hidden)]
pub fn __parse<T: VldParse + Type>(value: Option<Value>) -> ::poem_openapi::types::ParseResult<T> {
    T::vld_parse_value(&value.unwrap_or_default())
        .map_err(::poem_openapi::types::ParseError::custom)
}

/// A JSON request body validated with vld: **422** with every issue on
/// failure. See the [module docs](self).
#[derive(Debug, Clone, PartialEq)]
pub struct VldJson<T>(pub T);

impl<T> std::ops::Deref for VldJson<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: Type> Payload for VldJson<T> {
    const CONTENT_TYPE: &'static str = "application/json; charset=utf-8";

    fn check_content_type(content_type: &str) -> bool {
        <::poem_openapi::payload::Json<T> as Payload>::check_content_type(content_type)
    }

    fn schema_ref() -> MetaSchemaRef {
        T::schema_ref()
    }

    fn register(registry: &mut Registry) {
        T::register(registry);
    }
}

impl<T: Type + VldParse> ParsePayload for VldJson<T> {
    const IS_REQUIRED: bool = true;

    async fn from_request(request: &Request, body: &mut RequestBody) -> poem::Result<Self> {
        let data = Vec::<u8>::from_request(request, body).await?;
        let value: Value = serde_json::from_slice(&data).map_err(|e| {
            validation_error(&VldError::single(
                IssueCode::ParseError,
                format!("Invalid JSON: {e}"),
            ))
        })?;
        T::vld_parse_value(&value)
            .map(Self)
            .map_err(|e| validation_error(&e))
    }
}

::poem_openapi::impl_apirequest_for_payload!(VldJson<T>, T: Type + VldParse);

impl<T: serde::Serialize + Send> IntoResponse for VldJson<T> {
    fn into_response(self) -> Response {
        poem::web::Json(self.0).into_response()
    }
}

impl<T: Type + serde::Serialize> ApiResponse for VldJson<T> {
    fn meta() -> MetaResponses {
        MetaResponses {
            responses: vec![MetaResponse {
                description: "",
                status: Some(200),
                status_range: None,
                content: vec![MetaMediaType {
                    content_type: Self::CONTENT_TYPE,
                    schema: Self::schema_ref(),
                }],
                headers: vec![],
            }],
        }
    }

    fn register(registry: &mut Registry) {
        T::register(registry);
    }
}

/// The 422 response `VldJson` rejects with: `{"error": "Validation failed",
/// "issues": [{"path": ".name", "message": "...", "code": "too_small"}]}`.
pub fn validation_error(err: &VldError) -> poem::Error {
    let issues: Vec<Value> = err
        .issues
        .iter()
        .map(|i| json!({"path": i.path_string(), "message": i.message, "code": i.code.key()}))
        .collect();
    let body = json!({"error": "Validation failed", "issues": issues});
    poem::Error::from_response(
        Response::builder()
            .status(poem::http::StatusCode::UNPROCESSABLE_ENTITY)
            .content_type("application/json")
            .body(body.to_string()),
    )
}

/// Implement poem-openapi `Type` + `ParseFromJSON` for a vld type:
/// `impl_type!(T)` or `impl_type!(T, "ComponentName")`. See the
/// [module docs](self).
#[doc(hidden)]
#[macro_export]
macro_rules! __poem_openapi_impl_type {
    ($ty:ty) => {
        $crate::__poem_openapi_impl_type!($ty, stringify!($ty));
    };
    ($ty:ty, $name:expr) => {
        impl $crate::poem_openapi::poem_openapi::types::Type for $ty {
            const IS_REQUIRED: bool = true;
            type RawValueType = Self;
            type RawElementValueType = Self;

            fn name() -> ::std::borrow::Cow<'static, str> {
                ::std::borrow::Cow::Borrowed($name)
            }

            fn schema_ref() -> $crate::poem_openapi::poem_openapi::registry::MetaSchemaRef {
                $crate::poem_openapi::poem_openapi::registry::MetaSchemaRef::Reference(
                    ::std::string::String::from($name),
                )
            }

            fn register(registry: &mut $crate::poem_openapi::poem_openapi::registry::Registry) {
                // Inherent `__vld_nested_schemas` (from `vld::schema!` / derive) wins
                // over this trait fallback in `<T>::f()` path resolution.
                #[allow(unused_imports)]
                use $crate::__VldNestedSchemasFallback as _;
                $crate::poem_openapi::__register::<Self>(
                    registry,
                    $name,
                    <$ty>::json_schema,
                    <$ty>::__vld_nested_schemas(),
                );
            }

            fn as_raw_value(&self) -> ::std::option::Option<&Self> {
                ::std::option::Option::Some(self)
            }

            fn raw_element_iter<'a>(
                &'a self,
            ) -> ::std::boxed::Box<dyn ::std::iter::Iterator<Item = &'a Self> + 'a> {
                ::std::boxed::Box::new(::std::iter::once(self))
            }
        }

        impl $crate::poem_openapi::poem_openapi::types::ParseFromJSON for $ty {
            fn parse_from_json(
                value: ::std::option::Option<$crate::poem_openapi::__Value>,
            ) -> $crate::poem_openapi::poem_openapi::types::ParseResult<Self> {
                $crate::poem_openapi::__parse(value)
            }
        }
    };
}
#[doc(inline)]
pub use crate::__poem_openapi_impl_type as impl_type;

#[doc(hidden)]
pub use serde_json::Value as __Value;

/// [`impl_type!`], [`VldJson`], [`json_schema_to_meta`] and `vld::prelude`.
pub mod prelude {
    pub use super::{VldJson, impl_type, json_schema_to_meta};
    pub use vld::prelude::*;
}
