/// Generate `validate_fields()` and `parse_lenient()` methods for a struct
/// previously defined with [`schema!`](crate::schema!).
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
///   (uses `Default` for invalid fields), returns [`ParseResult<Self>`](crate::object::ParseResult)
///
/// The returned [`ParseResult`](crate::object::ParseResult) can be inspected,
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
        $crate::impl_validate_fields!(@keyed $name {
            $(
                $field_name : $field_type => [
                    $crate::__vld_resolve_key!(
                        $crate::__private::field_key(stringify!($field_name)) $(, $rename)?
                    )
                ] $schema
            ),*
        });
    };
    // Internal: every field with its JSON key expression.
    (
        @keyed $name:ident {
            $( $field_name:ident : $field_type:ty => [$key:expr] $schema:expr ),* $(,)?
        }
    ) => {
        impl $name {
            /// Validate each field individually and return per-field results.
            ///
            /// Unlike `parse()`, this does **not** fail fast — every field is
            /// validated and you see which fields passed and which failed.
            pub fn validate_fields<__VldInputT: $crate::schema::VldInput + ?Sized>(
                input: &__VldInputT,
            ) -> ::std::result::Result<
                ::std::vec::Vec<$crate::object::FieldResult>,
                $crate::error::VldError,
            > {
                let __vld_json = <__VldInputT as $crate::schema::VldInput>::to_json_value(input)?;
                Self::validate_fields_value(&__vld_json)
            }

            /// Validate each field individually from a `serde_json::Value`.
            pub fn validate_fields_value(
                __vld_json: &$crate::serde_json::Value,
            ) -> ::std::result::Result<
                ::std::vec::Vec<$crate::object::FieldResult>,
                $crate::error::VldError,
            > {
                #[allow(unused_imports)]
                use $crate::schema::VldSchema as _;
                let __vld_obj = $crate::__private::expect_object(__vld_json)?;
                ::std::result::Result::Ok(::std::vec![
                    $(
                        $crate::__private::field_result(
                            __vld_obj,
                            $key,
                            $crate::__vld_cached!($schema),
                        )
                    ),*
                ])
            }

            /// Parse leniently: build the struct even when some fields fail.
            ///
            /// - Valid fields get their parsed value.
            /// - Invalid fields fall back to `Default::default()`.
            ///
            /// Returns a [`ParseResult`](crate::object::ParseResult) that wraps
            /// the struct and per-field diagnostics. You can inspect it, convert
            /// to JSON, or save to a file whenever you need.
            pub fn parse_lenient<__VldInputT: $crate::schema::VldInput + ?Sized>(
                input: &__VldInputT,
            ) -> ::std::result::Result<
                $crate::object::ParseResult<$name>,
                $crate::error::VldError,
            > {
                let __vld_json = <__VldInputT as $crate::schema::VldInput>::to_json_value(input)?;
                Self::parse_lenient_value(&__vld_json)
            }

            /// Parse leniently from a `serde_json::Value`.
            pub fn parse_lenient_value(
                __vld_json: &$crate::serde_json::Value,
            ) -> ::std::result::Result<
                $crate::object::ParseResult<$name>,
                $crate::error::VldError,
            > {
                #[allow(unused_imports)]
                use $crate::schema::VldSchema as _;
                let __vld_obj = $crate::__private::expect_object(__vld_json)?;
                let mut __vld_results: ::std::vec::Vec<$crate::object::FieldResult> =
                    ::std::vec::Vec::new();

                $(
                    #[allow(non_snake_case)]
                    let $field_name: $field_type = $crate::__private::lenient_field(
                        __vld_obj,
                        $key,
                        $crate::__vld_cached!($schema),
                        &mut __vld_results,
                    );
                )*

                ::std::result::Result::Ok($crate::object::ParseResult::new(
                    $name { $( $field_name, )* },
                    __vld_results,
                ))
            }
        }
    };
}

/// Attach validation rules to an **existing** struct.
///
/// Unlike [`schema!`](crate::schema!) which creates the struct, this macro takes a struct you
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
                #[allow(unused_mut)]
                let mut __vld_failed = false;

                $(
                    {
                        let __vld_key: &str = $crate::__vld_resolve_key!(
                            $crate::__private::field_key(stringify!($field)) $(, $rename)?
                        );
                        let __vld_result = match $crate::serde_json::to_value(&self.$field) {
                            ::std::result::Result::Ok(ref __vld_val) => {
                                $crate::schema::VldSchema::parse_value($crate::__vld_cached!($schema), __vld_val)
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
                            __vld_failed = true;
                            __vld_errors.issues.extend(
                                e.with_prefix($crate::error::PathSegment::Field(
                                    ::std::string::String::from(__vld_key),
                                ))
                                .issues,
                            );
                        }
                    }
                )*

                if !__vld_failed {
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
