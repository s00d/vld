//! Macro bodies shared by the majors of one dependency. `$m` names the major's
//! module (`utoipa5`, `schemars1`, …); each module exports a thin wrapper
//! (`__utoipa5_impl_to_schema` re-exported as `utoipa5::impl_to_schema`) that
//! forwards here with its own name.

#[cfg(any(feature = "utoipa-5", feature = "utoipa-6"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __utoipa_impl_to_schema {
    (
        @impl $m:ident;
        $ty:ty,
        name: { $($name_method:tt)* },
        param_in: $param_in_override:expr $(,)?
    ) => {
        impl $crate::$m::utoipa::PartialSchema for $ty {
            fn schema() -> $crate::$m::utoipa::openapi::RefOr<$crate::$m::utoipa::openapi::schema::Schema> {
                $crate::$m::json_schema_to_schema(&<$ty>::json_schema())
            }
        }

        impl $crate::$m::utoipa::ToSchema for $ty {
            $($name_method)*

            fn schemas(
                schemas: &mut ::std::vec::Vec<(
                    ::std::string::String,
                    $crate::$m::utoipa::openapi::RefOr<$crate::$m::utoipa::openapi::schema::Schema>,
                )>,
            ) {
                // Inherent `__vld_nested_schemas` (from `vld::schema!` / derive) wins
                // over this trait fallback in `<T>::f()` path resolution.
                #[allow(unused_imports)]
                use $crate::__VldNestedSchemasFallback as _;
                for (name, schema_fn) in <$ty>::__vld_nested_schemas() {
                    let json = schema_fn();
                    schemas.push((name.to_string(), $crate::$m::json_schema_to_schema(&json)));
                }
            }
        }

        impl $crate::$m::utoipa::IntoParams for $ty {
            fn into_params(
                parameter_in_provider: impl Fn() -> Option<$crate::$m::utoipa::openapi::path::ParameterIn>,
            ) -> ::std::vec::Vec<$crate::$m::utoipa::openapi::path::Parameter> {
                let Some(parameter_in) = $crate::$m::resolve_parameter_in_with_override::<$ty>(
                    parameter_in_provider,
                    $param_in_override,
                ) else {
                    return ::std::vec::Vec::new();
                };
                $crate::$m::json_schema_to_params(&<$ty>::json_schema(), parameter_in)
            }
        }
    };
    ($m:ident; $ty:ty) => {
        $crate::__utoipa_impl_to_schema!(@impl $m; $ty, name: {}, param_in: None);
    };
    ($m:ident; $ty:ty, $name:literal) => {
        $crate::__utoipa_impl_to_schema!(
            @impl $m;
            $ty,
            name: {
                fn name() -> ::std::borrow::Cow<'static, str> {
                    ::std::borrow::Cow::Borrowed($name)
                }
            },
            param_in: None,
        );
    };
    ($m:ident; $ty:ty, query) => {
        $crate::__utoipa_impl_to_schema!(
            @impl $m;
            $ty,
            name: {},
            param_in: Some($crate::$m::utoipa::openapi::path::ParameterIn::Query),
        );
    };
    ($m:ident; $ty:ty, path) => {
        $crate::__utoipa_impl_to_schema!(
            @impl $m;
            $ty,
            name: {},
            param_in: Some($crate::$m::utoipa::openapi::path::ParameterIn::Path),
        );
    };
    ($m:ident; $ty:ty, header) => {
        $crate::__utoipa_impl_to_schema!(
            @impl $m;
            $ty,
            name: {},
            param_in: Some($crate::$m::utoipa::openapi::path::ParameterIn::Header),
        );
    };
    ($m:ident; $ty:ty, cookie) => {
        $crate::__utoipa_impl_to_schema!(
            @impl $m;
            $ty,
            name: {},
            param_in: Some($crate::$m::utoipa::openapi::path::ParameterIn::Cookie),
        );
    };
}

#[cfg(any(feature = "schemars-0", feature = "schemars-1"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __schemars_impl_json_schema {
    ($m:ident; $ty:ty) => {
        $crate::__schemars_impl_json_schema!($m; $ty, stringify!($ty));
    };
    ($m:ident; $ty:ty, $name:expr) => {
        impl $crate::$m::schemars::JsonSchema for $ty {
            fn schema_name() -> ::std::borrow::Cow<'static, str> {
                ::std::borrow::Cow::Borrowed($name)
            }

            fn schema_id() -> ::std::borrow::Cow<'static, str> {
                ::std::borrow::Cow::Owned(format!("{}::{}", module_path!(), $name))
            }

            fn json_schema(
                generator: &mut $crate::$m::schemars::SchemaGenerator,
            ) -> $crate::$m::schemars::Schema {
                // Inherent `__vld_nested_schemas` (from `vld::schema!` / derive) wins
                // over this trait fallback in `<T>::f()` path resolution.
                #[allow(unused_imports)]
                use $crate::__VldNestedSchemasFallback as _;
                $crate::$m::__vld_json_schema(
                    generator,
                    <$ty>::json_schema(),
                    <$ty>::__vld_nested_schemas(),
                )
            }
        }
    };
}

#[cfg(any(feature = "schemars-0", feature = "schemars-1"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __schemars_impl_vld_parse {
    ($m:ident; $ty:ty) => {
        impl $crate::vld::schema::VldParse for $ty {
            fn vld_parse_value(
                value: &::serde_json::Value,
            ) -> ::std::result::Result<Self, $crate::vld::error::VldError> {
                let schema = $crate::$m::generate_schemars::<$ty>();
                $crate::$m::validate_with_schemars(&schema, value)?;
                ::serde_json::from_value(value.clone()).map_err(|e| {
                    $crate::vld::error::VldError::single(
                        $crate::vld::error::IssueCode::ParseError,
                        format!("Deserialization error: {}", e),
                    )
                })
            }
        }

        impl $crate::$m::SchemarsValidate for $ty {}
    };
}
