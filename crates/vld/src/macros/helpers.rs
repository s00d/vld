//! Helper macros for the generated code.
//!
//! The feature-gate helpers are conditionally compiled based on **vld's**
//! features, so when a foreign crate invokes `vld::schema!`, the check uses
//! vld's features instead of the consumer's — eliminating `unexpected_cfgs`
//! warnings.

/// The schema built by `$schema`, cached process-wide on first use
/// (`&'static`). Only for non-generic code.
#[doc(hidden)]
#[macro_export]
macro_rules! __vld_cached {
    ($schema:expr) => {{
        static __VLD_CELL: $crate::__private::SchemaCell = $crate::__private::SchemaCell::new();
        __VLD_CELL.get(|| $schema)
    }};
}

/// Emit the given tokens only when the `openapi` feature is enabled on `vld`.
#[cfg(feature = "openapi")]
#[doc(hidden)]
#[macro_export]
macro_rules! __vld_if_openapi {
    ($($tt:tt)*) => { $($tt)* };
}

/// No-op: `openapi` feature is disabled.
#[cfg(not(feature = "openapi"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __vld_if_openapi {
    ($($tt:tt)*) => {};
}

#[cfg(feature = "openapi")]
#[doc(hidden)]
#[macro_export]
macro_rules! __vld_nested_schema_fn {
    ($ty:ty) => {
        Some(<$ty>::json_schema as fn() -> $crate::serde_json::Value)
    };
}

#[cfg(not(feature = "openapi"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __vld_nested_schema_fn {
    ($ty:ty) => {
        None
    };
}

#[cfg(feature = "openapi")]
#[doc(hidden)]
#[macro_export]
macro_rules! __vld_nested_collect_fn {
    ($ty:ty) => {
        Some(
            <$ty>::__vld_collect_nested_schemas
                as fn(&mut ::std::vec::Vec<$crate::openapi::NestedSchemaEntry>),
        )
    };
}

#[cfg(not(feature = "openapi"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __vld_nested_collect_fn {
    ($ty:ty) => {
        None
    };
}
