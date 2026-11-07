/// Helper macro to resolve a field's JSON key.
///
/// If a rename literal is provided, use it; otherwise fall back to the field name.
#[doc(hidden)]
#[macro_export]
macro_rules! __vld_resolve_key {
    ($default:expr) => {
        $default
    };
    ($default:expr, $override:expr) => {
        $override
    };
}

/// Filter `#[into_params(...)]` off struct attributes, then emit attrs + items together.
#[doc(hidden)]
#[macro_export]
macro_rules! __vld_struct_attrs {
    ( attrs: $(#[ $($attr:tt)* ])*
      body: { $($body:tt)* }
    ) => {
        $crate::__vld_struct_attrs!(@filter [] $(#[ $($attr)* ])*
            body: { $($body)* }
        );
    };
    (
        @filter [$($acc:tt)*]
        #[ into_params ( $($p:tt)* ) ]
        $(#[ $($rest:tt)* ])*
        body: { $($body:tt)* }
    ) => {
        $crate::__vld_struct_attrs!(@filter [$($acc)*]
            $(#[ $($rest)* ])*
            body: { $($body)* }
        );
    };
    (
        @filter [$($acc:tt)*]
        #[ $($attr:tt)* ]
        $(#[ $($rest:tt)* ])*
        body: { $($body:tt)* }
    ) => {
        $crate::__vld_struct_attrs!(@filter [$($acc)* #[ $($attr)* ]]
            $(#[ $($rest)* ])*
            body: { $($body)* }
        );
    };
    (
        @filter [$($acc:tt)*]
        body: { $($body:tt)* }
    ) => {
        $($acc)*
        $($body)*
    };
}

/// Define a validated struct with field-level schemas.
///
/// This macro generates:
/// - A regular Rust struct with the specified fields and types
/// - A `parse()` method that validates input and constructs the struct
/// - A `parse_value()` method for direct `serde_json::Value` input
/// - An implementation of [`VldParse`](crate::schema::VldParse) for use with framework extractors
///
/// # Syntax
///
/// ```ignore
/// vld::schema! {
///     #[derive(Debug, Clone)]
///     pub struct MyStruct {
///         pub field_name: FieldType => schema_expression,
///         pub renamed_field: FieldType as "jsonKey" => schema_expression,
///         // ...
///     }
/// }
/// ```
///
/// Each field has the format: `name: Type [as "json_key"] => schema`.
/// The optional `as "json_key"` overrides the JSON property name used for parsing.
///
/// # Serde renames
///
/// Without `as "json_key"`, the key follows the serde attributes written in
/// the macro: field `#[serde(rename = "...")]` /
/// `#[serde(rename(serialize = "...", deserialize = "..."))]`, then struct
/// `#[serde(rename_all = "...")]` (also the `serialize` / `deserialize`
/// form; same conventions as serde). `parse()` / `parse_value()` / JSON
/// Schema use the **deserialize** names; `validate()` / `is_valid()`
/// serialize the value and check it under the **serialize** names, so a
/// `Serialize`d instance validates. `as "json_key"` sets both names (serde
/// does not see it: pair it with a matching `#[serde(rename)]` if needed).
/// `alias`, `flatten`, `skip` and `default` are not interpreted.
///
/// # Schema caching
///
/// Every field schema expression is evaluated **once per process** (on first
/// use) and cached in a `static`, so it must be `Send + Sync + 'static`, and
/// a schema built from runtime values (`vld::string().max(config.limit)`)
/// freezes the values seen on the first parse. Build such schemas at runtime
/// with [`vld::object()`](crate::object()) instead.
///
/// # OpenAPI parameters
///
/// Raw identifiers map to their bare name (`r#type` → JSON key `"type"`).
///
/// For query/path/header/cookie parameters, add utoipa's attribute on the struct
/// (same as with `#[derive(Validate)]`), then call `vld_utoipa::impl_to_schema!` once:
///
/// ```ignore
/// vld::schema! {
///     #[derive(Debug)]
///     #[into_params(parameter_in = Query)]
///     pub struct SearchQuery {
///         pub q: String => vld::string().min(1),
///     }
/// }
/// impl_to_schema!(SearchQuery);
/// ```
///
/// # Example
///
/// ```
/// use vld::prelude::*;
///
/// vld::schema! {
///     #[derive(Debug)]
///     pub struct User {
///         pub name: String => vld::string().min(2).max(50),
///         pub age: Option<i64> => vld::number().int().min(0).optional(),
///         pub tags: Vec<String> => vld::array(vld::string()).max_len(5)
///             .with_default(vec![]),
///     }
/// }
///
/// let user = User::parse(r#"{"name": "Alice", "age": 30}"#).unwrap();
/// assert_eq!(user.name, "Alice");
/// assert_eq!(user.age, Some(30));
/// assert!(user.tags.is_empty());
/// ```
///
/// # Renamed Fields
///
/// ```ignore
/// vld::schema! {
///     pub struct ApiResponse {
///         pub first_name: String as "firstName" => vld::string().min(1),
///         pub last_name: String as "lastName" => vld::string().min(1),
///     }
/// }
///
/// // Parses from camelCase JSON:
/// let r = ApiResponse::parse(r#"{"firstName": "John", "lastName": "Doe"}"#).unwrap();
/// assert_eq!(r.first_name, "John");
/// ```
///
/// # Nested Structs
///
/// Use [`nested()`](crate::nested()) to compose schemas:
///
/// ```ignore
/// vld::schema! {
///     pub struct Address {
///         pub city: String => vld::string().min(1),
///     }
/// }
///
/// vld::schema! {
///     pub struct User {
///         pub name: String => vld::string(),
///         pub address: Address => vld::nested(Address::parse_value),
///     }
/// }
/// ```
#[macro_export]
macro_rules! schema {
    (
        $(#[ $($struct_attr:tt)* ])*
        $vis:vis struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                $field_vis:vis $field_name:ident : $field_type:ty $(as $rename:literal)? => $schema:expr
            ),* $(,)?
        }
    ) => {
        $crate::__vld_struct_attrs! {
            attrs: $(#[ $($struct_attr)* ])*
            body: {
        $vis struct $name {
            $(
                $(#[$field_meta])*
                $field_vis $field_name: $field_type,
            )*
        }

        impl $name {
            /// Parse and validate input data into this struct.
            ///
            /// Accepts any type implementing [`VldInput`]: JSON strings, file paths,
            /// `serde_json::Value`, byte slices, etc.
            pub fn parse<__VldInputT: $crate::schema::VldInput + ?Sized>(
                input: &__VldInputT,
            ) -> ::std::result::Result<$name, $crate::error::VldError> {
                let __vld_json = <__VldInputT as $crate::schema::VldInput>::to_json_value(input)?;
                Self::parse_value(&__vld_json)
            }

            /// Parse and validate directly from a `serde_json::Value`.
            pub fn parse_value(
                __vld_json: &$crate::serde_json::Value,
            ) -> ::std::result::Result<$name, $crate::error::VldError> {
                Self::__vld_parse(__vld_json, false)
            }

            /// Same as [`parse_value`](Self::parse_value); callable as
            /// `T::vld_parse_value` without importing `vld::schema::VldParse`.
            pub fn vld_parse_value(
                __vld_json: &$crate::serde_json::Value,
            ) -> ::std::result::Result<$name, $crate::error::VldError> {
                Self::__vld_parse(__vld_json, false)
            }

            /// JSON key of a field: deserialize name (`ser == false`, parsing)
            /// or serialize name (`ser == true`, `validate()`).
            #[doc(hidden)]
            pub fn __vld_key(__vld_field: &str, __vld_ser: bool) -> &'static str {
                static __VLD_KEYS: ::std::sync::OnceLock<
                    ::std::vec::Vec<(&'static str, ::std::string::String, ::std::string::String)>,
                > = ::std::sync::OnceLock::new();
                let __vld_keys = __VLD_KEYS.get_or_init(|| {
                    #[allow(unused_variables)]
                    let __vld_struct_attrs: &[&str] = &[$(stringify!($($struct_attr)*)),*];
                    ::std::vec![$(
                        $crate::__private::serde_keys(
                            stringify!($field_name),
                            $crate::__vld_resolve_key!(None $(, Some($rename))?),
                            &[$(stringify!($field_meta)),*],
                            __vld_struct_attrs,
                        )
                    ),*]
                });
                $crate::__private::key_of(__vld_keys, __vld_field, __vld_ser)
            }

            #[doc(hidden)]
            pub fn __vld_parse(
                __vld_json: &$crate::serde_json::Value,
                __vld_ser: bool,
            ) -> ::std::result::Result<$name, $crate::error::VldError> {
                #[allow(unused_imports)]
                use $crate::schema::VldSchema as _;
                let __vld_obj = $crate::__private::expect_object(__vld_json)?;
                let mut __vld_errors = $crate::error::VldError::new();

                $(
                    #[allow(non_snake_case)]
                    let $field_name: ::std::option::Option<$field_type> =
                        $crate::__private::parse_field(
                            __vld_obj,
                            Self::__vld_key(stringify!($field_name), __vld_ser),
                            $crate::__vld_cached!($schema),
                            &mut __vld_errors,
                        );
                )*

                // A field may fail with an issue-less error: check the values too.
                if !__vld_errors.is_empty() $(|| $field_name.is_none())* {
                    return ::std::result::Result::Err(__vld_errors);
                }

                ::std::result::Result::Ok($name {
                    $(
                        $field_name: $field_name.unwrap(),
                    )*
                })
            }
        }

        impl $crate::schema::VldParse for $name {
            fn vld_parse_value(
                value: &$crate::serde_json::Value,
            ) -> ::std::result::Result<Self, $crate::error::VldError> {
                Self::parse_value(value)
            }
        }

        impl $name {
            /// Validate an existing Rust value that can be serialized to JSON.
            ///
            /// The value is serialized via `serde`, then validated against the
            /// schema under the fields' **serialize** names (`parse()` reads
            /// the deserialize names). Returns `Ok(())` on success,
            /// `Err(VldError)` with all issues on failure.
            pub fn validate<__VldT: $crate::serde::Serialize>(
                instance: &__VldT,
            ) -> ::std::result::Result<(), $crate::error::VldError> {
                Self::__vld_parse(&$crate::__private::to_json(instance)?, true).map(|_| ())
            }

            /// Check if a value is valid against the schema.
            ///
            /// Shorthand for `validate(instance).is_ok()`.
            pub fn is_valid<__VldT: $crate::serde::Serialize>(instance: &__VldT) -> bool {
                Self::validate(instance).is_ok()
            }
        }

        $crate::__vld_if_openapi! {
            impl $name {
                /// Generate a JSON Schema / OpenAPI 3.1 representation of this struct.
                ///
                /// Requires the `openapi` feature.
                pub fn json_schema() -> $crate::serde_json::Value {
                    #[allow(unused_imports)]
                    use $crate::schema::VldSchema as _;
                    #[allow(unused_imports)]
                    use $crate::openapi::JsonSchema as _;
                    static __VLD_JSON_SCHEMA: $crate::__private::JsonSchemaCell =
                        $crate::__private::JsonSchemaCell::new();
                    __VLD_JSON_SCHEMA.get_or_init(|| $crate::__private::object_schema(::std::vec![
                        $(
                            {
                                let __vld_field_schema = $schema;
                                (
                                    Self::__vld_key(stringify!($field_name), false),
                                    __vld_field_schema.json_schema(),
                                    __vld_field_schema.is_required(),
                                )
                            }
                        ),*
                    ])).clone()
                }

                /// Minimal OpenAPI 3.1 document with this struct and every
                /// nested schema it references under `components.schemas`.
                ///
                /// Requires the `openapi` feature.
                pub fn to_openapi_document() -> $crate::serde_json::Value {
                    $crate::__private::openapi_document(
                        stringify!($name),
                        Self::json_schema(),
                        Self::__vld_nested_schemas(),
                    )
                }

                /// Collect `(name, json_schema_fn)` pairs for all nested schemas
                /// used by fields of this struct into `out`.
                ///
                /// Recurses transitively (with name dedup) so grandchild schemas
                /// are registered for OpenAPI components.
                #[doc(hidden)]
                pub fn __vld_collect_nested_schemas(
                    __vld_out: &mut ::std::vec::Vec<$crate::openapi::NestedSchemaEntry>,
                ) {
                    #[allow(unused_imports)]
                    use $crate::schema::VldSchema as _;
                    $(
                        $crate::openapi::CollectNestedSchemas::collect_nested_schemas(
                            &$schema,
                            __vld_out,
                        );
                    )*
                }

                /// Collect `(name, json_schema_fn)` pairs for all nested schemas
                /// used by fields of this struct.
                ///
                /// Used by `vld-utoipa`'s `impl_to_schema!` to automatically register
                /// nested types as OpenAPI components.
                #[doc(hidden)]
                pub fn __vld_nested_schemas()
                    -> ::std::vec::Vec<$crate::openapi::NestedSchemaEntry>
                {
                    let mut __vld_out: ::std::vec::Vec<$crate::openapi::NestedSchemaEntry> =
                        ::std::vec::Vec::new();
                    Self::__vld_collect_nested_schemas(&mut __vld_out);
                    __vld_out
                }
            }

            impl $crate::openapi::OpenApiParameterIn for $name {
                fn parameter_in() -> Option<&'static str> {
                    $crate::schema!(@param_in $(#[ $($struct_attr)* ])*)
                }
            }
        }
            }
        }
    };

    // Parse `#[into_params(..., parameter_in = ..., ...)]` for [`OpenApiParameterIn`].
    (@param_in) => {
        None::<&'static str>
    };
    (@param_in #[ into_params ( $($p:tt)* ) ] $(#[ $($rest:tt)* ])*) => {
        match $crate::schema!(@param_in_args $($p)*) {
            Some(__vld_in) => Some(__vld_in),
            None => $crate::schema!(@param_in $(#[ $($rest)* ])*),
        }
    };
    (@param_in #[ $($skip:tt)* ] $(#[ $($rest:tt)* ])*) => {
        $crate::schema!(@param_in $(#[ $($rest)* ])*)
    };
    // Scan the `into_params(...)` argument list one token tree at a time.
    (@param_in_args) => { None::<&'static str> };
    (@param_in_args parameter_in = Query $($rest:tt)*) => { Some("query") };
    (@param_in_args parameter_in = Path $($rest:tt)*) => { Some("path") };
    (@param_in_args parameter_in = Header $($rest:tt)*) => { Some("header") };
    (@param_in_args parameter_in = Cookie $($rest:tt)*) => { Some("cookie") };
    (@param_in_args $skip:tt $($rest:tt)*) => { $crate::schema!(@param_in_args $($rest)*) };
}

/// Combined macro: generates the struct, `parse()`, **and** `validate_fields()` /
/// `parse_lenient()` in a single declaration — no need to repeat field schemas.
///
/// This is equivalent to calling `schema!` + `impl_validate_fields!` together.
///
/// **Extra requirements** compared to `schema!`:
/// - All field types must implement `serde::Serialize` (for per-field JSON output)
/// - All field types must implement `Default` (for lenient fallback values)
///
/// # Example
///
/// ```ignore
/// vld::schema_validated! {
///     #[derive(Debug, serde::Serialize)]
///     pub struct User {
///         pub name: String => vld::string().min(2),
///         pub age: Option<i64> => vld::number().int().optional(),
///     }
/// }
///
/// // Has parse(), validate_fields(), parse_lenient(), etc.
/// let result = User::parse_lenient(r#"{"name":"X"}"#).unwrap();
/// result.save_to_file(std::path::Path::new("out.json")).unwrap();
/// ```
#[macro_export]
macro_rules! schema_validated {
    (
        $(#[ $($struct_attr:tt)* ])*
        $vis:vis struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                $field_vis:vis $field_name:ident : $field_type:ty $(as $rename:literal)? => $schema:expr
            ),* $(,)?
        }
    ) => {
        // 1. Generate the struct + parse/parse_value (same as schema!)
        $crate::schema! {
            $(#[ $($struct_attr)* ])*
            $vis struct $name {
                $(
                    $(#[$field_meta])*
                    $field_vis $field_name : $field_type $(as $rename)? => $schema
                ),*
            }
        }

        // 2. Generate validate_fields + parse_lenient (same as impl_validate_fields!)
        $crate::impl_validate_fields!(@keyed $name {
            $( $field_name : $field_type => [<$name>::__vld_key(stringify!($field_name), false)] $schema ),*
        });
    };
}

/// Create a named nested schema with `$ref` generation and full JSON Schema.
///
/// Works with types from [`schema!`] and `#[derive(Validate)]`. The component
/// name is the type as written, sanitized to `[A-Za-z0-9._-]`
/// (`Page<User>` → `Page_User_`).
///
/// ```ignore
/// vld::schema! {
///     pub struct User {
///         pub address: Address => vld::nested!(Address),
///     }
/// }
/// ```
#[macro_export]
macro_rules! nested {
    ($ty:ty) => {
        $crate::nested_named(
            {
                static __VLD_NAME: ::std::sync::OnceLock<::std::string::String> =
                    ::std::sync::OnceLock::new();
                __VLD_NAME
                    .get_or_init(|| $crate::__private::component_name(stringify!($ty)))
                    .as_str()
            },
            <$ty>::parse_value,
            $crate::__vld_nested_schema_fn!($ty),
            $crate::__vld_nested_collect_fn!($ty),
        )
    };
}
