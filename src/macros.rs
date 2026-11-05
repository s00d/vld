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
/// # OpenAPI parameters
///
/// Raw identifiers map to their bare name (`r#type` → JSON key `"type"`).
///
/// For query/path/header/cookie parameters, add utoipa's attribute on the struct
/// (same as with `#[derive(Validate)]`), then call [`vld_utoipa::impl_to_schema`] once:
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
/// Use [`nested()`](crate::nested) to compose schemas:
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
            pub fn parse<__VldInputT: $crate::input::VldInput + ?Sized>(
                input: &__VldInputT,
            ) -> ::std::result::Result<$name, $crate::error::VldError> {
                let __vld_json = <__VldInputT as $crate::input::VldInput>::to_json_value(input)?;
                Self::parse_value(&__vld_json)
            }

            /// Parse and validate directly from a `serde_json::Value`.
            pub fn parse_value(
                __vld_json: &$crate::serde_json::Value,
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
                            $crate::__vld_resolve_key!(
                                $crate::__private::field_key(stringify!($field_name)) $(, $rename)?
                            ),
                            &$schema,
                            &mut __vld_errors,
                        );
                )*

                if !__vld_errors.is_empty() {
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

        $crate::__vld_if_serialize! {
            impl $name {
                /// Validate an existing Rust value that can be serialized to JSON.
                ///
                /// The value is serialized via `serde`, then validated against the
                /// schema. Returns `Ok(())` on success, `Err(VldError)` with all
                /// issues on failure.
                ///
                /// Requires the `serialize` feature.
                pub fn validate<__VldT: $crate::serde::Serialize>(
                    instance: &__VldT,
                ) -> ::std::result::Result<(), $crate::error::VldError> {
                    Self::parse_value(&$crate::__private::to_json(instance)?).map(|_| ())
                }

                /// Check if a value is valid against the schema.
                ///
                /// Shorthand for `validate(instance).is_ok()`.
                ///
                /// Requires the `serialize` feature.
                pub fn is_valid<__VldT: $crate::serde::Serialize>(instance: &__VldT) -> bool {
                    Self::validate(instance).is_ok()
                }
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
                    use $crate::json_schema::JsonSchema as _;
                    $crate::__private::object_schema(::std::vec![
                        $(
                            {
                                let __vld_field_schema = $schema;
                                (
                                    $crate::__vld_resolve_key!(
                                        $crate::__private::field_key(stringify!($field_name))
                                        $(, $rename)?
                                    ),
                                    __vld_field_schema.json_schema(),
                                    __vld_field_schema.is_required(),
                                )
                            }
                        ),*
                    ])
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
                    __vld_out: &mut ::std::vec::Vec<$crate::json_schema::NestedSchemaEntry>,
                ) {
                    #[allow(unused_imports)]
                    use $crate::schema::VldSchema as _;
                    $(
                        $crate::json_schema::CollectNestedSchemas::collect_nested_schemas(
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
                    -> ::std::vec::Vec<$crate::json_schema::NestedSchemaEntry>
                {
                    let mut __vld_out: ::std::vec::Vec<$crate::json_schema::NestedSchemaEntry> =
                        ::std::vec::Vec::new();
                    Self::__vld_collect_nested_schemas(&mut __vld_out);
                    __vld_out
                }
            }

            impl $crate::json_schema::OpenApiParameterIn for $name {
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

/// Generate `validate_fields()` and `parse_lenient()` methods for a struct
/// previously defined with [`schema!`].
///
/// Syntax mirrors `schema!`, but without visibility/attributes:
///
/// ```ignore
/// vld::impl_validate_fields!(User {
///     name: String => vld::string().min(2),
///     age: i64     => vld::number().int(),
/// });
/// ```
///
/// Fields can also use `as "json_key"` to match a renamed JSON property:
///
/// ```ignore
/// vld::impl_validate_fields!(User {
///     first_name: String as "firstName" => vld::string().min(2),
/// });
/// ```
///
/// Generated methods:
///
/// - **`validate_fields(input)`** — validate each field, return `Vec<FieldResult>`
/// - **`parse_lenient(input)`** — build the struct even if some fields fail
///   (uses `Default` for invalid fields), returns [`ParseResult<Self>`](crate::error::ParseResult)
///
/// The returned [`ParseResult`](crate::error::ParseResult) can be inspected,
/// converted to JSON, or saved to a file at any time via `.save_to_file(path)`.
///
/// **Requires:**
/// - Field output types: `serde::Serialize`
/// - For `parse_lenient`: field types also need `Default`
/// - For `save_to_file` / `to_json_string`: the struct needs `serde::Serialize`
#[macro_export]
macro_rules! impl_validate_fields {
    (
        $name:ident {
            $( $field_name:ident : $field_type:ty $(as $rename:literal)? => $schema:expr ),* $(,)?
        }
    ) => {
        impl $name {
            /// Validate each field individually and return per-field results.
            ///
            /// Unlike `parse()`, this does **not** fail fast — every field is
            /// validated and you see which fields passed and which failed.
            pub fn validate_fields<__VldInputT: $crate::input::VldInput + ?Sized>(
                input: &__VldInputT,
            ) -> ::std::result::Result<
                ::std::vec::Vec<$crate::error::FieldResult>,
                $crate::error::VldError,
            > {
                let __vld_json = <__VldInputT as $crate::input::VldInput>::to_json_value(input)?;
                Self::validate_fields_value(&__vld_json)
            }

            /// Validate each field individually from a `serde_json::Value`.
            pub fn validate_fields_value(
                __vld_json: &$crate::serde_json::Value,
            ) -> ::std::result::Result<
                ::std::vec::Vec<$crate::error::FieldResult>,
                $crate::error::VldError,
            > {
                #[allow(unused_imports)]
                use $crate::schema::VldSchema as _;
                let __vld_obj = $crate::__private::expect_object(__vld_json)?;
                ::std::result::Result::Ok(::std::vec![
                    $(
                        $crate::__private::field_result(
                            __vld_obj,
                            $crate::__vld_resolve_key!(
                                $crate::__private::field_key(stringify!($field_name)) $(, $rename)?
                            ),
                            &$schema,
                        )
                    ),*
                ])
            }

            /// Parse leniently: build the struct even when some fields fail.
            ///
            /// - Valid fields get their parsed value.
            /// - Invalid fields fall back to `Default::default()`.
            ///
            /// Returns a [`ParseResult`](crate::error::ParseResult) that wraps
            /// the struct and per-field diagnostics. You can inspect it, convert
            /// to JSON, or save to a file whenever you need.
            pub fn parse_lenient<__VldInputT: $crate::input::VldInput + ?Sized>(
                input: &__VldInputT,
            ) -> ::std::result::Result<
                $crate::error::ParseResult<$name>,
                $crate::error::VldError,
            > {
                let __vld_json = <__VldInputT as $crate::input::VldInput>::to_json_value(input)?;
                Self::parse_lenient_value(&__vld_json)
            }

            /// Parse leniently from a `serde_json::Value`.
            pub fn parse_lenient_value(
                __vld_json: &$crate::serde_json::Value,
            ) -> ::std::result::Result<
                $crate::error::ParseResult<$name>,
                $crate::error::VldError,
            > {
                #[allow(unused_imports)]
                use $crate::schema::VldSchema as _;
                let __vld_obj = $crate::__private::expect_object(__vld_json)?;
                let mut __vld_results: ::std::vec::Vec<$crate::error::FieldResult> =
                    ::std::vec::Vec::new();

                $(
                    #[allow(non_snake_case)]
                    let $field_name: $field_type = $crate::__private::lenient_field(
                        __vld_obj,
                        $crate::__vld_resolve_key!(
                            $crate::__private::field_key(stringify!($field_name)) $(, $rename)?
                        ),
                        &$schema,
                        &mut __vld_results,
                    );
                )*

                ::std::result::Result::Ok($crate::error::ParseResult::new(
                    $name { $( $field_name, )* },
                    __vld_results,
                ))
            }
        }
    };
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
        $crate::impl_validate_fields!($name {
            $( $field_name : $field_type $(as $rename)? => $schema ),*
        });
    };
}

/// Attach validation rules to an **existing** struct.
///
/// Unlike [`schema!`] which creates the struct, this macro takes a struct you
/// already have and generates `validate()` and `is_valid()` instance methods.
///
/// The struct must implement `serde::Serialize`.
///
/// The struct does **not** need `#[derive(Serialize)]` or `#[derive(Debug)]` —
/// each field is serialized individually (standard types like `String`, `f64`,
/// `Vec<T>` already implement `Serialize`).
///
/// # Example
///
/// ```
/// use vld::prelude::*;
///
/// // No Serialize or Debug required on the struct itself
/// struct Product {
///     name: String,
///     price: f64,
///     tags: Vec<String>,
/// }
///
/// vld::impl_rules!(Product {
///     name => vld::string().min(2).max(100),
///     price => vld::number().positive(),
///     tags => vld::array(vld::string().min(1)).max_len(10),
/// });
///
/// let p = Product {
///     name: "Widget".into(),
///     price: 9.99,
///     tags: vec!["sale".into()],
/// };
/// assert!(p.is_valid());
///
/// let bad = Product {
///     name: "X".into(),
///     price: -1.0,
///     tags: vec![],
/// };
/// assert!(!bad.is_valid());
/// let err = bad.validate().unwrap_err();
/// assert!(err.issues.len() >= 2);
/// ```
///
/// Use `field as "wireName" => schema` to report errors under the JSON name
/// (e.g. when the struct uses `#[serde(rename = ...)]`).
#[macro_export]
macro_rules! impl_rules {
    (
        $name:ident {
            $( $field:ident $(as $rename:literal)? => $schema:expr ),* $(,)?
        }
    ) => {
        impl $name {
            /// Validate this instance against the declared rules.
            ///
            /// Each field is serialized to JSON individually and checked
            /// against its schema. All errors are accumulated.
            pub fn validate(&self) -> ::std::result::Result<(), $crate::error::VldError> {
                #[allow(unused_imports)]
                use $crate::schema::VldSchema as _;
                let mut __vld_errors = $crate::error::VldError::new();

                $(
                    {
                        let __vld_key: &str = $crate::__vld_resolve_key!(
                            $crate::__private::field_key(stringify!($field)) $(, $rename)?
                        );
                        let __vld_result = match $crate::serde_json::to_value(&self.$field) {
                            ::std::result::Result::Ok(ref __vld_val) => {
                                $crate::schema::VldSchema::parse_value(&$schema, __vld_val)
                                    .map(|_| ())
                            }
                            ::std::result::Result::Err(e) => ::std::result::Result::Err(
                                $crate::error::VldError::single(
                                    $crate::error::IssueCode::ParseError,
                                    ::std::format!(
                                        "Serialization error for field '{}': {}",
                                        __vld_key, e
                                    ),
                                ),
                            ),
                        };
                        if let ::std::result::Result::Err(e) = __vld_result {
                            __vld_errors.issues.extend(
                                e.with_prefix($crate::error::PathSegment::Field(
                                    ::std::string::String::from(__vld_key),
                                ))
                                .issues,
                            );
                        }
                    }
                )*

                if __vld_errors.is_empty() {
                    ::std::result::Result::Ok(())
                } else {
                    ::std::result::Result::Err(__vld_errors)
                }
            }

            /// Check if this instance passes all validation rules.
            pub fn is_valid(&self) -> bool {
                self.validate().is_ok()
            }
        }
    };
}

/// Generate `impl Default` for a struct created by [`schema!`].
///
/// Every listed field is set to **its type's** `Default::default()` — the same
/// as `#[derive(Default)]`. Schema defaults (`.with_default(...)`) are **not**
/// used; to get them, parse an empty object instead:
/// `Config::parse_value(&serde_json::json!({}))`.
///
/// # Example
///
/// ```
/// use vld::prelude::*;
///
/// vld::schema! {
///     #[derive(Debug)]
///     pub struct Config {
///         pub host: String => vld::string().with_default("localhost".into()),
///         pub port: Option<i64> => vld::number().int().optional(),
///         pub tags: Vec<String> => vld::array(vld::string()).with_default(vec![]),
///     }
/// }
///
/// vld::impl_default!(Config { host, port, tags });
///
/// let cfg = Config::default();
/// assert_eq!(cfg.host, "");       // String::default(), NOT the schema default
/// assert_eq!(cfg.port, None);     // Option::default()
/// assert!(cfg.tags.is_empty());   // Vec::default()
///
/// // Schema defaults come from parsing:
/// let parsed = Config::parse_value(&serde_json::json!({})).unwrap();
/// assert_eq!(parsed.host, "localhost");
/// ```
#[macro_export]
macro_rules! impl_default {
    ($name:ident { $($field:ident),* $(,)? }) => {
        impl ::std::default::Default for $name {
            fn default() -> Self {
                Self {
                    $( $field: ::std::default::Default::default(), )*
                }
            }
        }
    };
}

/// Create a union schema from 2 or more schemas.
///
/// Dispatches to `vld::union()` for 2 schemas and `vld::union3()` for 3.
/// For 4+ schemas, unions are nested automatically (any number of schemas;
/// order and first-match semantics are preserved).
///
/// # Examples
///
/// ```rust
/// use vld::prelude::*;
///
/// // 2 schemas
/// let s = vld::union!(vld::string(), vld::number().int());
/// assert!(s.parse(r#""hello""#).is_ok());
/// assert!(s.parse("42").is_ok());
///
/// // 3 schemas
/// let s = vld::union!(vld::string(), vld::number().int(), vld::boolean());
/// assert!(s.parse("true").is_ok());
/// ```
#[macro_export]
macro_rules! union {
    // 2 schemas
    ($a:expr, $b:expr $(,)?) => {
        $crate::union($a, $b)
    };
    // 3 schemas
    ($a:expr, $b:expr, $c:expr $(,)?) => {
        $crate::union3($a, $b, $c)
    };
    // 4 schemas — nest as union(union(a, b), union(c, d))
    ($a:expr, $b:expr, $c:expr, $d:expr $(,)?) => {
        $crate::union($crate::union($a, $b), $crate::union($c, $d))
    };
    // 5 schemas
    ($a:expr, $b:expr, $c:expr, $d:expr, $e:expr $(,)?) => {
        $crate::union($crate::union3($a, $b, $c), $crate::union($d, $e))
    };
    // 6 schemas
    ($a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr $(,)?) => {
        $crate::union($crate::union3($a, $b, $c), $crate::union3($d, $e, $f))
    };
    // 7+ schemas — first six, then the rest recursively
    ($a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr, $($rest:expr),+ $(,)?) => {
        $crate::union(
            $crate::union!($a, $b, $c, $d, $e, $f),
            $crate::union!($($rest),+),
        )
    };
}
