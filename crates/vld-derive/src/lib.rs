use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Expr, Fields, Lit, Meta};

/// Derive macro that generates `vld_parse()`, `parse_value()`, `validate_fields()`,
/// and `parse_lenient()` methods for a struct, plus implements the `VldParse` trait.
///
/// # Usage
///
/// ```ignore
/// use vld::Validate;
///
/// #[derive(Debug, Validate)]
/// struct User {
///     #[vld(vld::string().min(2).max(50))]
///     name: String,
///     #[vld(vld::string().email())]
///     email: String,
///     #[vld(vld::number().int().gte(18).optional())]
///     age: Option<i64>,
/// }
///
/// let user = User::vld_parse(r#"{"name": "Alex", "email": "a@b.com"}"#).unwrap();
/// ```
///
/// # Serde rename support
///
/// The derive macro respects `#[serde(rename = "...")]` on fields and
/// `#[serde(rename_all = "...")]` on the struct:
///
/// ```ignore
/// #[derive(Debug, serde::Serialize, Validate)]
/// #[serde(rename_all = "camelCase")]
/// struct ApiRequest {
///     #[vld(vld::string().min(2))]
///     first_name: String,
///     #[vld(vld::string().email())]
///     email_address: String,
/// }
/// // Parses from {"firstName": "...", "emailAddress": "..."}
/// ```
///
/// Supported rename_all conventions: `camelCase`, `PascalCase`, `snake_case`,
/// `SCREAMING_SNAKE_CASE`, `kebab-case`, `SCREAMING-KEBAB-CASE`.
///
/// The expression inside `#[vld(...)]` is used as-is in the generated code.
/// Make sure the types are in scope (e.g., use `vld::string()` or import via prelude).
#[proc_macro_derive(Validate, attributes(vld))]
pub fn derive_validate(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Check for #[serde(rename_all = "...")]
    let rename_all = get_serde_rename_all(&input.attrs);

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("Validate can only be derived for structs with named fields"),
        },
        _ => panic!("Validate can only be derived for structs"),
    };

    let mut field_names = Vec::new();
    let mut field_types = Vec::new();
    let mut field_schemas = Vec::new();
    let mut field_json_keys = Vec::new();

    for field in fields {
        let fname = field.ident.as_ref().unwrap();
        let ftype = &field.ty;
        field_names.push(fname.clone());
        field_types.push(ftype.clone());

        // Determine JSON key: #[serde(rename = "...")] > rename_all > field name
        let json_key = get_serde_rename(&field.attrs).unwrap_or_else(|| {
            if let Some(ref convention) = rename_all {
                rename_field(&fname.to_string(), convention)
            } else {
                fname.to_string()
            }
        });
        field_json_keys.push(json_key);

        let schema_tokens = field
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("vld"))
            .map(|attr| attr.parse_args::<proc_macro2::TokenStream>().unwrap())
            .unwrap_or_else(|| panic!("Field `{}` is missing #[vld(...)] attribute", fname));

        field_schemas.push(schema_tokens);
    }

    let expanded = quote! {
        impl #name {
            /// Parse and validate input data into this struct.
            ///
            /// Named `vld_parse` to avoid conflicts with other derive macros
            /// (e.g. `clap::Parser::parse()`).
            pub fn vld_parse<__VldInputT: ::vld::input::VldInput + ?Sized>(
                input: &__VldInputT,
            ) -> ::std::result::Result<#name, ::vld::error::VldError> {
                let __vld_json = <__VldInputT as ::vld::input::VldInput>::to_json_value(input)?;
                Self::parse_value(&__vld_json)
            }

            /// Parse and validate directly from a `serde_json::Value`.
            pub fn parse_value(
                __vld_json: &::vld::serde_json::Value,
            ) -> ::std::result::Result<#name, ::vld::error::VldError> {
                use ::vld::schema::VldSchema as _;

                let __vld_obj = __vld_json.as_object().ok_or_else(|| {
                    ::vld::error::VldError::single(
                        ::vld::error::IssueCode::InvalidType {
                            expected: ::std::string::String::from("object"),
                            received: ::vld::error::value_type_name(__vld_json),
                        },
                        ::std::format!(
                            "Expected object, received {}",
                            ::vld::error::value_type_name(__vld_json)
                        ),
                    )
                })?;

                let mut __vld_errors = ::vld::error::VldError::new();

                #(
                    #[allow(non_snake_case)]
                    let #field_names: ::std::option::Option<#field_types> = {
                        let __vld_field_schema = { #field_schemas };
                        let __vld_field_value = __vld_obj
                            .get(#field_json_keys)
                            .unwrap_or(&::vld::serde_json::Value::Null);
                        match __vld_field_schema.parse_value(__vld_field_value) {
                            ::std::result::Result::Ok(v) => ::std::option::Option::Some(v),
                            ::std::result::Result::Err(e) => {
                                __vld_errors = ::vld::error::VldError::merge(
                                    __vld_errors,
                                    ::vld::error::VldError::with_prefix(
                                        e,
                                        ::vld::error::PathSegment::Field(
                                            ::std::string::String::from(#field_json_keys),
                                        ),
                                    ),
                                );
                                ::std::option::Option::None
                            }
                        }
                    };
                )*

                if !::vld::error::VldError::is_empty(&__vld_errors) {
                    return ::std::result::Result::Err(__vld_errors);
                }

                ::std::result::Result::Ok(#name {
                    #( #field_names: #field_names.unwrap(), )*
                })
            }

            /// Validate each field individually and return per-field results.
            pub fn validate_fields<__VldInputT: ::vld::input::VldInput + ?Sized>(
                input: &__VldInputT,
            ) -> ::std::result::Result<
                ::std::vec::Vec<::vld::error::FieldResult>,
                ::vld::error::VldError,
            > {
                let __vld_json = <__VldInputT as ::vld::input::VldInput>::to_json_value(input)?;
                Self::validate_fields_value(&__vld_json)
            }

            /// Validate each field individually from a `serde_json::Value`.
            pub fn validate_fields_value(
                __vld_json: &::vld::serde_json::Value,
            ) -> ::std::result::Result<
                ::std::vec::Vec<::vld::error::FieldResult>,
                ::vld::error::VldError,
            > {
                let __vld_obj = __vld_json.as_object().ok_or_else(|| {
                    ::vld::error::VldError::single(
                        ::vld::error::IssueCode::InvalidType {
                            expected: ::std::string::String::from("object"),
                            received: ::vld::error::value_type_name(__vld_json),
                        },
                        ::std::format!(
                            "Expected object, received {}",
                            ::vld::error::value_type_name(__vld_json)
                        ),
                    )
                })?;

                let mut __vld_results: ::std::vec::Vec<::vld::error::FieldResult> =
                    ::std::vec::Vec::new();

                #(
                    {
                        let __vld_field_schema = { #field_schemas };
                        let __vld_field_value = __vld_obj
                            .get(#field_json_keys)
                            .unwrap_or(&::vld::serde_json::Value::Null);

                        let __vld_result = ::vld::object::DynSchema::dyn_parse(
                            &__vld_field_schema,
                            __vld_field_value,
                        );

                        __vld_results.push(::vld::error::FieldResult {
                            name: ::std::string::String::from(#field_json_keys),
                            input: __vld_field_value.clone(),
                            result: __vld_result,
                        });
                    }
                )*

                ::std::result::Result::Ok(__vld_results)
            }

            /// Parse leniently: build the struct even when some fields fail.
            pub fn parse_lenient<__VldInputT: ::vld::input::VldInput + ?Sized>(
                input: &__VldInputT,
            ) -> ::std::result::Result<
                ::vld::error::ParseResult<#name>,
                ::vld::error::VldError,
            > {
                let __vld_json = <__VldInputT as ::vld::input::VldInput>::to_json_value(input)?;
                Self::parse_lenient_value(&__vld_json)
            }

            /// Parse leniently from a `serde_json::Value`.
            pub fn parse_lenient_value(
                __vld_json: &::vld::serde_json::Value,
            ) -> ::std::result::Result<
                ::vld::error::ParseResult<#name>,
                ::vld::error::VldError,
            > {
                use ::vld::schema::VldSchema as _;

                let __vld_obj = __vld_json.as_object().ok_or_else(|| {
                    ::vld::error::VldError::single(
                        ::vld::error::IssueCode::InvalidType {
                            expected: ::std::string::String::from("object"),
                            received: ::vld::error::value_type_name(__vld_json),
                        },
                        ::std::format!(
                            "Expected object, received {}",
                            ::vld::error::value_type_name(__vld_json)
                        ),
                    )
                })?;

                let mut __vld_results: ::std::vec::Vec<::vld::error::FieldResult> =
                    ::std::vec::Vec::new();

                #(
                    #[allow(non_snake_case)]
                    let #field_names: #field_types = {
                        let __vld_field_schema = { #field_schemas };
                        let __vld_field_value = __vld_obj
                            .get(#field_json_keys)
                            .unwrap_or(&::vld::serde_json::Value::Null);

                        match __vld_field_schema.parse_value(__vld_field_value) {
                            ::std::result::Result::Ok(v) => {
                                let __json_repr = ::vld::serde_json::to_value(&v)
                                    .unwrap_or_else(|_| __vld_field_value.clone());
                                __vld_results.push(::vld::error::FieldResult {
                                    name: ::std::string::String::from(#field_json_keys),
                                    input: __vld_field_value.clone(),
                                    result: ::std::result::Result::Ok(__json_repr),
                                });
                                v
                            }
                            ::std::result::Result::Err(e) => {
                                __vld_results.push(::vld::error::FieldResult {
                                    name: ::std::string::String::from(#field_json_keys),
                                    input: __vld_field_value.clone(),
                                    result: ::std::result::Result::Err(e),
                                });
                                <#field_types as ::std::default::Default>::default()
                            }
                        }
                    };
                )*

                let __vld_struct = #name {
                    #( #field_names, )*
                };

                ::std::result::Result::Ok(
                    ::vld::error::ParseResult::new(__vld_struct, __vld_results)
                )
            }
        }

        impl ::vld::schema::VldParse for #name {
            fn vld_parse_value(
                value: &::vld::serde_json::Value,
            ) -> ::std::result::Result<Self, ::vld::error::VldError> {
                Self::parse_value(value)
            }
        }
    };

    TokenStream::from(expanded)
}

// ---------------------------------------------------------------------------
// Serde attribute parsing helpers
// ---------------------------------------------------------------------------

/// Extract `#[serde(rename_all = "...")]` from struct-level attributes.
fn get_serde_rename_all(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        if let Ok(nested) = attr
            .parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)
        {
            for meta in &nested {
                if let Meta::NameValue(nv) = meta {
                    if nv.path.is_ident("rename_all") {
                        if let Expr::Lit(lit) = &nv.value {
                            if let Lit::Str(s) = &lit.lit {
                                return Some(s.value());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Extract `#[serde(rename = "...")]` from field-level attributes.
fn get_serde_rename(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        if let Ok(nested) = attr
            .parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)
        {
            for meta in &nested {
                if let Meta::NameValue(nv) = meta {
                    if nv.path.is_ident("rename") {
                        if let Expr::Lit(lit) = &nv.value {
                            if let Lit::Str(s) = &lit.lit {
                                return Some(s.value());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Convert a snake_case field name to the given naming convention.
fn rename_field(name: &str, convention: &str) -> String {
    match convention {
        "camelCase" => to_camel_case(name),
        "PascalCase" => to_pascal_case(name),
        "snake_case" => name.to_string(),
        "SCREAMING_SNAKE_CASE" => name.to_uppercase(),
        "kebab-case" => name.replace('_', "-"),
        "SCREAMING-KEBAB-CASE" => name.replace('_', "-").to_uppercase(),
        _ => name.to_string(),
    }
}

fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    for ch in s.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.extend(ch.to_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }
    result
}

fn to_pascal_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;
    for ch in s.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.extend(ch.to_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }
    result
}
