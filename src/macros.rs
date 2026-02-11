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
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                $field_vis:vis $field_name:ident : $field_type:ty $(as $rename:literal)? => $schema:expr
            ),* $(,)?
        }
    ) => {
        $(#[$meta])*
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
                use $crate::schema::VldSchema as _;

                let __vld_obj = __vld_json.as_object().ok_or_else(|| {
                    $crate::error::VldError::single(
                        $crate::error::IssueCode::InvalidType {
                            expected: ::std::string::String::from("object"),
                            received: $crate::error::value_type_name(__vld_json),
                        },
                        ::std::format!(
                            "Expected object, received {}",
                            $crate::error::value_type_name(__vld_json)
                        ),
                    )
                })?;

                let mut __vld_errors = $crate::error::VldError::new();

                $(
                    #[allow(non_snake_case)]
                    let $field_name: ::std::option::Option<$field_type> = {
                        let __vld_field_schema = $schema;
                        let __vld_key = $crate::__vld_resolve_key!(
                            stringify!($field_name) $(, $rename)?
                        );
                        let __vld_field_value = __vld_obj
                            .get(__vld_key)
                            .unwrap_or(&$crate::serde_json::Value::Null);
                        match __vld_field_schema.parse_value(__vld_field_value) {
                            ::std::result::Result::Ok(v) => ::std::option::Option::Some(v),
                            ::std::result::Result::Err(e) => {
                                __vld_errors = $crate::error::VldError::merge(
                                    __vld_errors,
                                    $crate::error::VldError::with_prefix(
                                        e,
                                        $crate::error::PathSegment::Field(
                                            ::std::string::String::from(__vld_key),
                                        ),
                                    ),
                                );
                                ::std::option::Option::None
                            }
                        }
                    };
                )*

                if !$crate::error::VldError::is_empty(&__vld_errors) {
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
                    let __vld_json = $crate::serde_json::to_value(instance).map_err(|e| {
                        $crate::error::VldError::single(
                            $crate::error::IssueCode::ParseError,
                            ::std::format!("Serialization error: {}", e),
                        )
                    })?;
                    let _ = Self::parse_value(&__vld_json)?;
                    ::std::result::Result::Ok(())
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
                    use $crate::json_schema::JsonSchema as _;
                    let mut __vld_properties = $crate::serde_json::Map::new();
                    let mut __vld_required: ::std::vec::Vec<::std::string::String> =
                        ::std::vec::Vec::new();

                    $(
                        {
                            let __vld_field_schema = $schema;
                            let __vld_key = $crate::__vld_resolve_key!(
                                stringify!($field_name) $(, $rename)?
                            );
                            __vld_properties.insert(
                                ::std::string::String::from(__vld_key),
                                __vld_field_schema.json_schema(),
                            );
                            __vld_required.push(
                                ::std::string::String::from(__vld_key),
                            );
                        }
                    )*

                    $crate::serde_json::json!({
                        "type": "object",
                        "required": __vld_required,
                        "properties": $crate::serde_json::Value::Object(__vld_properties),
                    })
                }

                /// Wrap `json_schema()` in a minimal OpenAPI 3.1 document.
                ///
                /// Requires the `openapi` feature.
                pub fn to_openapi_document() -> $crate::serde_json::Value {
                    $crate::json_schema::to_openapi_document(stringify!($name), &Self::json_schema())
                }
            }
        }
    };
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
                let __vld_obj = __vld_json.as_object().ok_or_else(|| {
                    $crate::error::VldError::single(
                        $crate::error::IssueCode::InvalidType {
                            expected: ::std::string::String::from("object"),
                            received: $crate::error::value_type_name(__vld_json),
                        },
                        ::std::format!(
                            "Expected object, received {}",
                            $crate::error::value_type_name(__vld_json)
                        ),
                    )
                })?;

                let mut __vld_results: ::std::vec::Vec<$crate::error::FieldResult> =
                    ::std::vec::Vec::new();

                $(
                    {
                        let __vld_field_schema = $schema;
                        let __vld_key = $crate::__vld_resolve_key!(
                            stringify!($field_name) $(, $rename)?
                        );
                        let __vld_field_value = __vld_obj
                            .get(__vld_key)
                            .unwrap_or(&$crate::serde_json::Value::Null);

                        let __vld_result = $crate::object::DynSchema::dyn_parse(
                            &__vld_field_schema,
                            __vld_field_value,
                        );

                        __vld_results.push($crate::error::FieldResult {
                            name: ::std::string::String::from(__vld_key),
                            input: __vld_field_value.clone(),
                            result: __vld_result,
                        });
                    }
                )*

                ::std::result::Result::Ok(__vld_results)
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
                use $crate::schema::VldSchema as _;

                let __vld_obj = __vld_json.as_object().ok_or_else(|| {
                    $crate::error::VldError::single(
                        $crate::error::IssueCode::InvalidType {
                            expected: ::std::string::String::from("object"),
                            received: $crate::error::value_type_name(__vld_json),
                        },
                        ::std::format!(
                            "Expected object, received {}",
                            $crate::error::value_type_name(__vld_json)
                        ),
                    )
                })?;

                let mut __vld_results: ::std::vec::Vec<$crate::error::FieldResult> =
                    ::std::vec::Vec::new();

                $(
                    #[allow(non_snake_case)]
                    let $field_name: $field_type = {
                        let __vld_field_schema = $schema;
                        let __vld_key = $crate::__vld_resolve_key!(
                            stringify!($field_name) $(, $rename)?
                        );
                        let __vld_field_value = __vld_obj
                            .get(__vld_key)
                            .unwrap_or(&$crate::serde_json::Value::Null);

                        match __vld_field_schema.parse_value(__vld_field_value) {
                            ::std::result::Result::Ok(v) => {
                                let __json_repr = $crate::serde_json::to_value(&v)
                                    .unwrap_or_else(|_| __vld_field_value.clone());
                                __vld_results.push($crate::error::FieldResult {
                                    name: ::std::string::String::from(__vld_key),
                                    input: __vld_field_value.clone(),
                                    result: ::std::result::Result::Ok(__json_repr),
                                });
                                v
                            }
                            ::std::result::Result::Err(e) => {
                                __vld_results.push($crate::error::FieldResult {
                                    name: ::std::string::String::from(__vld_key),
                                    input: __vld_field_value.clone(),
                                    result: ::std::result::Result::Err(e),
                                });
                                <$field_type as ::std::default::Default>::default()
                            }
                        }
                    };
                )*

                let __vld_struct = $name {
                    $( $field_name, )*
                };

                ::std::result::Result::Ok(
                    $crate::error::ParseResult::new(__vld_struct, __vld_results)
                )
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
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                $field_vis:vis $field_name:ident : $field_type:ty $(as $rename:literal)? => $schema:expr
            ),* $(,)?
        }
    ) => {
        // 1. Generate the struct + parse/parse_value (same as schema!)
        $crate::schema! {
            $(#[$meta])*
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
#[macro_export]
macro_rules! impl_rules {
    (
        $name:ident {
            $( $field:ident => $schema:expr ),* $(,)?
        }
    ) => {
        impl $name {
            /// Validate this instance against the declared rules.
            ///
            /// Each field is serialized to JSON individually and checked
            /// against its schema. All errors are accumulated.
            pub fn validate(&self) -> ::std::result::Result<(), $crate::error::VldError> {
                use $crate::schema::VldSchema as _;
                let mut __vld_errors = $crate::error::VldError::new();

                $(
                    {
                        let __vld_field_json = $crate::serde_json::to_value(&self.$field)
                            .map_err(|e| {
                                $crate::error::VldError::single(
                                    $crate::error::IssueCode::ParseError,
                                    ::std::format!(
                                        "Serialization error for field '{}': {}",
                                        stringify!($field), e
                                    ),
                                )
                            });
                        match __vld_field_json {
                            ::std::result::Result::Ok(ref __vld_val) => {
                                let __vld_field_schema = $schema;
                                if let ::std::result::Result::Err(e) =
                                    __vld_field_schema.parse_value(__vld_val)
                                {
                                    __vld_errors = $crate::error::VldError::merge(
                                        __vld_errors,
                                        $crate::error::VldError::with_prefix(
                                            e,
                                            $crate::error::PathSegment::Field(
                                                ::std::string::String::from(stringify!($field)),
                                            ),
                                        ),
                                    );
                                }
                            }
                            ::std::result::Result::Err(e) => {
                                __vld_errors = $crate::error::VldError::merge(
                                    __vld_errors,
                                    $crate::error::VldError::with_prefix(
                                        e,
                                        $crate::error::PathSegment::Field(
                                            ::std::string::String::from(stringify!($field)),
                                        ),
                                    ),
                                );
                            }
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
/// Use this instead of `#[derive(Default)]` to automatically generate a
/// `Default` implementation bounded on all field types implementing `Default`.
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
/// assert_eq!(cfg.host, "");       // String::default()
/// assert_eq!(cfg.port, None);     // Option::default()
/// assert!(cfg.tags.is_empty());   // Vec::default()
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
/// For 4+ schemas, unions are nested automatically.
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
}
