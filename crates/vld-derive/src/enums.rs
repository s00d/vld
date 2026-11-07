//! `#[derive(Validate)]` on enums, following serde's representations:
//! externally tagged (default), internally tagged (`tag`), adjacently tagged
//! (`tag` + `content`) and `untagged`.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{DataEnum, DeriveInput, Fields, LitStr};

use crate::{
    FieldInfo, Names, RENAME_RULES, collect_tokens, field_parse_lets, named_field_infos, names,
    object_schema_tokens, schema_ref, serde_attrs, skip_meta,
};

#[derive(Default)]
struct SerdeEnum {
    tag: Option<String>,
    content: Option<String>,
    untagged: bool,
    /// Variant names.
    rename_all: Names,
    /// Field names of every struct variant.
    rename_all_fields: Names,
}

#[derive(Default)]
struct SerdeVariant {
    rename: Names,
    aliases: Vec<String>,
    skip: bool,
    other: bool,
    /// Field names of this struct variant.
    rename_all: Names,
}

enum Shape {
    Unit,
    /// One unnamed field: its schema.
    Newtype(TokenStream2),
    /// Several unnamed fields: their schemas.
    Tuple(Vec<TokenStream2>),
    Struct(Vec<FieldInfo>),
}

struct Variant {
    ident: syn::Ident,
    /// Deserialize name plus aliases (parsing, JSON Schema).
    de_names: Vec<String>,
    /// Serialize name (`validate()`).
    ser_name: String,
    other: bool,
    shape: Shape,
}

pub(crate) fn expand_enum(input: &DeriveInput, data: &DataEnum) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let generic = !input.generics.params.is_empty();
    let serde = parse_serde_enum_attrs(&input.attrs)?;
    if let Some(attr) = input.attrs.iter().find(|a| a.path().is_ident("vld")) {
        return Err(syn::Error::new(
            attr.span(),
            "vld: no enum-level #[vld(...)] attributes are supported (`lenient` is struct-only)",
        ));
    }
    if serde.content.is_some() && serde.tag.is_none() {
        return Err(syn::Error::new(
            name.span(),
            "vld: #[serde(content = ...)] needs #[serde(tag = ...)]",
        ));
    }

    let mut variants = Vec::new();
    for v in &data.variants {
        let attrs = parse_serde_variant_attrs(&v.attrs)?;
        if attrs.skip {
            continue;
        }
        let bare = v.ident.to_string();
        let pick = |own: Option<String>, rule: &Option<String>| match own {
            Some(n) => n,
            None => match rule {
                Some(conv) => rename_variant(&bare, conv),
                None => bare.clone(),
            },
        };
        let mut de_names = vec![pick(attrs.rename.0, &serde.rename_all.0)];
        de_names.extend(attrs.aliases);
        let ser_name = pick(attrs.rename.1, &serde.rename_all.1);

        let field_schema = |f: &syn::Field| -> syn::Result<Option<TokenStream2>> {
            f.attrs
                .iter()
                .find(|a| a.path().is_ident("vld"))
                .map(|a| a.parse_args::<TokenStream2>())
                .transpose()
        };
        let shape = match &v.fields {
            Fields::Unit => Shape::Unit,
            Fields::Unnamed(f) if f.unnamed.len() == 1 => {
                let field = &f.unnamed[0];
                let ty = &field.ty;
                // A derived (or `schema!`) inner type validates itself.
                Shape::Newtype(
                    field_schema(field)?.unwrap_or_else(|| quote! { ::vld::nested!(#ty) }),
                )
            }
            Fields::Unnamed(f) => {
                if serde.tag.is_some() && serde.content.is_none() && !serde.untagged {
                    return Err(syn::Error::new(
                        v.span(),
                        "vld: tuple variants cannot be internally tagged (serde rejects them too)",
                    ));
                }
                let mut items = Vec::new();
                for field in &f.unnamed {
                    items.push(field_schema(field)?.ok_or_else(|| {
                        syn::Error::new(
                            field.span(),
                            "tuple variant field is missing a #[vld(...)] attribute",
                        )
                    })?);
                }
                Shape::Tuple(items)
            }
            Fields::Named(f) => {
                let rule = if attrs.rename_all.0.is_some() || attrs.rename_all.1.is_some() {
                    &attrs.rename_all
                } else {
                    &serde.rename_all_fields
                };
                Shape::Struct(named_field_infos(&f.named, rule, false)?)
            }
        };
        if attrs.other && !matches!(shape, Shape::Unit) {
            return Err(syn::Error::new(
                v.span(),
                "vld: #[serde(other)] must be on a unit variant",
            ));
        }
        if attrs.other && (serde.tag.is_none() || serde.untagged) {
            return Err(syn::Error::new(
                v.span(),
                "vld: #[serde(other)] needs an internally or adjacently tagged enum",
            ));
        }
        variants.push(Variant {
            ident: v.ident.clone(),
            de_names,
            ser_name,
            other: attrs.other,
            shape,
        });
    }

    let parse_body = parse_body(&serde, &variants, generic);
    let schema_body = schema_body(&serde, &variants);
    let collect_calls = collect_tokens(variants.iter().flat_map(|v| match &v.shape {
        Shape::Unit => Vec::new(),
        Shape::Newtype(s) => vec![s],
        Shape::Tuple(items) => items.iter().collect(),
        Shape::Struct(infos) => infos.iter().filter_map(|f| f.schema.as_ref()).collect(),
    }));
    let json_schema_body = if generic {
        schema_body
    } else {
        quote! {
            static __VLD_JSON_SCHEMA: ::vld::__private::JsonSchemaCell =
                ::vld::__private::JsonSchemaCell::new();
            __VLD_JSON_SCHEMA.get_or_init(|| #schema_body).clone()
        }
    };

    Ok(quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            /// Parse and validate input data into this enum.
            pub fn vld_parse<__VldInputT: ::vld::schema::VldInput + ?Sized>(
                input: &__VldInputT,
            ) -> ::std::result::Result<Self, ::vld::error::VldError> {
                let __vld_json = <__VldInputT as ::vld::schema::VldInput>::to_json_value(input)?;
                Self::parse_value(&__vld_json)
            }

            /// Parse and validate directly from a `serde_json::Value`.
            pub fn parse_value(
                __vld_json: &::vld::serde_json::Value,
            ) -> ::std::result::Result<Self, ::vld::error::VldError> {
                Self::__vld_parse(__vld_json, false)
            }

            /// Parse under the deserialize (`false`) or serialize (`true`) names.
            #[doc(hidden)]
            #[allow(unused_variables, unreachable_code, clippy::needless_return)]
            pub fn __vld_parse(
                __vld_json: &::vld::serde_json::Value,
                __vld_ser: bool,
            ) -> ::std::result::Result<Self, ::vld::error::VldError> {
                #[allow(unused_imports)]
                use ::vld::schema::VldSchema as _;
                #parse_body
            }

            /// Validate an existing value that can be serialized to JSON
            /// (checked under the serialize names).
            pub fn validate<__VldT: ::vld::serde::Serialize>(
                instance: &__VldT,
            ) -> ::std::result::Result<(), ::vld::error::VldError> {
                Self::__vld_parse(&::vld::__private::to_json(instance)?, true).map(|_| ())
            }

            /// Shorthand for `validate(instance).is_ok()`.
            pub fn is_valid<__VldT: ::vld::serde::Serialize>(instance: &__VldT) -> bool {
                Self::validate(instance).is_ok()
            }
        }

        impl #impl_generics ::vld::schema::VldParse for #name #ty_generics #where_clause {
            fn vld_parse_value(
                value: &::vld::serde_json::Value,
            ) -> ::std::result::Result<Self, ::vld::error::VldError> {
                Self::parse_value(value)
            }
        }

        ::vld::__vld_if_openapi! {
            impl #impl_generics #name #ty_generics #where_clause {
                /// JSON Schema / OpenAPI 3.1 representation of this enum (`oneOf`,
                /// with a `discriminator` when tagged; `anyOf` when untagged).
                ///
                /// Requires the `openapi` feature on `vld`.
                pub fn json_schema() -> ::vld::serde_json::Value {
                    #[allow(unused_imports)]
                    use ::vld::schema::VldSchema as _;
                    #[allow(unused_imports)]
                    use ::vld::openapi::JsonSchema as _;
                    #json_schema_body
                }

                /// Minimal OpenAPI 3.1 document with this enum and every
                /// nested schema it references under `components.schemas`.
                ///
                /// Requires the `openapi` feature on `vld`.
                pub fn to_openapi_document() -> ::vld::serde_json::Value {
                    ::vld::__private::openapi_document(
                        stringify!(#name),
                        Self::json_schema(),
                        Self::__vld_nested_schemas(),
                    )
                }

                #[doc(hidden)]
                pub fn __vld_collect_nested_schemas(
                    __vld_out: &mut ::std::vec::Vec<::vld::openapi::NestedSchemaEntry>,
                ) {
                    #[allow(unused_imports)]
                    use ::vld::schema::VldSchema as _;
                    #( #collect_calls )*
                }

                #[doc(hidden)]
                pub fn __vld_nested_schemas()
                    -> ::std::vec::Vec<::vld::openapi::NestedSchemaEntry>
                {
                    let mut __vld_out = ::std::vec::Vec::new();
                    Self::__vld_collect_nested_schemas(&mut __vld_out);
                    __vld_out
                }
            }

            impl #impl_generics ::vld::openapi::OpenApiParameterIn for #name #ty_generics #where_clause {
                fn parameter_in() -> Option<&'static str> {
                    None
                }
            }
        }
    })
}

/// `true` when `__vld_name` is this variant's name (under `__vld_ser`).
fn name_match(v: &Variant) -> TokenStream2 {
    let ser = &v.ser_name;
    let de = &v.de_names;
    if de.len() == 1 && de[0] == *ser {
        quote! { __vld_name == #ser }
    } else {
        quote! { (if __vld_ser { __vld_name == #ser } else { false #( || __vld_name == #de )* }) }
    }
}

/// `&[...]` of the accepted names, for `invalid_discriminator`.
fn options(variants: &[Variant]) -> TokenStream2 {
    let de = variants.iter().filter(|v| !v.other).map(|v| &v.de_names[0]);
    let ser = variants.iter().filter(|v| !v.other).map(|v| &v.ser_name);
    quote! {{
        let __vld_options: &[&str] = if __vld_ser { &[ #( #ser ),* ] } else { &[ #( #de ),* ] };
        __vld_options
    }}
}

/// Expression `Result<Self, VldError>` parsing the variant's content from
/// `__vld_content: &Value` (unused for unit variants).
fn content_parse(v: &Variant, generic: bool) -> TokenStream2 {
    let ident = &v.ident;
    match &v.shape {
        Shape::Unit => quote! { ::std::result::Result::Ok(Self::#ident) },
        Shape::Newtype(schema) => {
            let s = schema_ref(generic, schema);
            quote! { ::vld::schema::VldSchema::parse_value(#s, __vld_content).map(Self::#ident) }
        }
        Shape::Tuple(items) => {
            let n = items.len();
            let vars: Vec<_> = (0..n).map(|i| format_ident!("__vld_item{}", i)).collect();
            let lets = items.iter().zip(&vars).enumerate().map(|(i, (schema, var))| {
                let s = schema_ref(generic, schema);
                quote! {
                    let #var = ::vld::__private::parse_item(__vld_items, #i, #s, &mut __vld_errors);
                }
            });
            quote! {
                (|| {
                    let __vld_items = ::vld::__private::expect_tuple(__vld_content, #n)?;
                    let mut __vld_errors = ::vld::error::VldError::new();
                    #( #lets )*
                    if !__vld_errors.is_empty() #( || #vars.is_none() )* {
                        return ::std::result::Result::Err(__vld_errors);
                    }
                    ::std::result::Result::Ok(Self::#ident( #( #vars.unwrap() ),* ))
                })()
            }
        }
        Shape::Struct(infos) => {
            let lets = field_parse_lets(infos, generic);
            let fields: Vec<_> = infos.iter().map(|f| &f.ident).collect();
            quote! {
                (|| {
                    let __vld_obj = ::vld::__private::expect_object(__vld_content)?;
                    let mut __vld_errors = ::vld::error::VldError::new();
                    #( #lets )*
                    if !__vld_errors.is_empty() #( || #fields.is_none() )* {
                        return ::std::result::Result::Err(__vld_errors);
                    }
                    ::std::result::Result::Ok(Self::#ident { #( #fields: #fields.unwrap(), )* })
                })()
            }
        }
    }
}

fn parse_body(serde: &SerdeEnum, variants: &[Variant], generic: bool) -> TokenStream2 {
    let opts = options(variants);
    let field = |name: TokenStream2| {
        quote! { ::vld::error::PathSegment::Field(::std::string::ToString::to_string(#name)) }
    };

    if serde.untagged {
        let tries = variants.iter().map(|v| {
            let ident = &v.ident;
            let attempt = match v.shape {
                Shape::Unit => quote! {
                    ::vld::__private::expect_unit(::std::option::Option::Some(__vld_json)).map(|_| Self::#ident)
                },
                _ => {
                    let parse = content_parse(v, generic);
                    quote! { { let __vld_content = __vld_json; #parse } }
                }
            };
            quote! {
                match #attempt {
                    ::std::result::Result::Ok(__vld_v) => return ::std::result::Result::Ok(__vld_v),
                    ::std::result::Result::Err(__vld_e) => __vld_failures.push(__vld_e),
                }
            }
        });
        return quote! {
            let mut __vld_failures = ::std::vec::Vec::new();
            #( #tries )*
            ::std::result::Result::Err(::vld::__private::untagged_error(__vld_json, __vld_failures))
        };
    }

    match (&serde.tag, &serde.content) {
        (None, _) => {
            let arms = variants.iter().map(|v| {
                let m = name_match(v);
                let key = field(quote! { __vld_name });
                let body = match v.shape {
                    Shape::Unit => {
                        let ident = &v.ident;
                        quote! { ::vld::__private::expect_unit(__vld_inner).map(|_| Self::#ident) }
                    }
                    _ => {
                        let parse = content_parse(v, generic);
                        quote! {{
                            let __vld_content = __vld_inner.unwrap_or(&::vld::serde_json::Value::Null);
                            #parse
                        }}
                    }
                };
                quote! { if #m { return (#body).map_err(|e| e.with_prefix(#key)); } }
            });
            quote! {
                let (__vld_name, __vld_inner) = ::vld::__private::external_variant(__vld_json)?;
                #( #arms )*
                ::std::result::Result::Err(::vld::__private::unknown_variant(
                    ::std::option::Option::None, __vld_name, #opts,
                ))
            }
        }
        (Some(tag), None) => {
            let arms = variants.iter().filter(|v| !v.other).map(|v| {
                let m = name_match(v);
                let body = match v.shape {
                    Shape::Newtype(_) => {
                        let parse = content_parse(v, generic);
                        quote! {{
                            let __vld_rest = ::vld::__private::without_key(__vld_obj, #tag);
                            let __vld_content = &__vld_rest;
                            #parse
                        }}
                    }
                    _ => {
                        let parse = content_parse(v, generic);
                        quote! {{ let __vld_content = __vld_json; #parse }}
                    }
                };
                quote! { if #m { return #body; } }
            });
            let fallback = other_fallback(variants, tag, &opts);
            quote! {
                let __vld_obj = ::vld::__private::expect_object(__vld_json)?;
                let __vld_name = ::vld::__private::enum_tag(__vld_obj, #tag)?;
                #( #arms )*
                #fallback
            }
        }
        (Some(tag), Some(content)) => {
            let key = field(quote! { #content });
            let arms = variants.iter().filter(|v| !v.other).map(|v| {
                let m = name_match(v);
                let body = match v.shape {
                    Shape::Unit => {
                        let ident = &v.ident;
                        quote! {
                            ::vld::__private::expect_unit(__vld_obj.get(#content))
                                .map(|_| Self::#ident)
                                .map_err(|e| e.with_prefix(#key))
                        }
                    }
                    _ => {
                        let parse = content_parse(v, generic);
                        quote! {{
                            let __vld_content = __vld_obj.get(#content).ok_or_else(|| {
                                ::vld::error::VldError::from_code(::vld::error::IssueCode::MissingField)
                                    .with_prefix(#key)
                            })?;
                            (#parse).map_err(|e| e.with_prefix(#key))
                        }}
                    }
                };
                quote! { if #m { return #body; } }
            });
            let fallback = other_fallback(variants, tag, &opts);
            quote! {
                let __vld_obj = ::vld::__private::expect_object(__vld_json)?;
                let __vld_name = ::vld::__private::enum_tag(__vld_obj, #tag)?;
                #( #arms )*
                #fallback
            }
        }
    }
}

/// `#[serde(other)]` variant for unknown tags, or the `invalid_discriminator` error.
fn other_fallback(variants: &[Variant], tag: &str, opts: &TokenStream2) -> TokenStream2 {
    match variants.iter().find(|v| v.other) {
        Some(v) => {
            let ident = &v.ident;
            quote! { ::std::result::Result::Ok(Self::#ident) }
        }
        None => quote! {
            ::std::result::Result::Err(::vld::__private::unknown_variant(
                ::std::option::Option::Some(#tag), __vld_name, #opts,
            ))
        },
    }
}

fn schema_body(serde: &SerdeEnum, variants: &[Variant]) -> TokenStream2 {
    let entries = variants.iter().filter(|v| !v.other).map(|v| {
        let name = &v.de_names[0];
        let schema = match &v.shape {
            Shape::Unit => quote! { ::vld::__private::VariantSchema::Unit },
            Shape::Newtype(s) => {
                quote! { ::vld::__private::VariantSchema::Value({ #s }.json_schema()) }
            }
            Shape::Tuple(items) => quote! {
                ::vld::__private::VariantSchema::Value(::vld::__private::tuple_schema(
                    ::std::vec![ #( { #items }.json_schema() ),* ],
                ))
            },
            Shape::Struct(infos) => {
                let obj = object_schema_tokens(infos);
                quote! { ::vld::__private::VariantSchema::Object(#obj) }
            }
        };
        quote! { (#name, #schema) }
    });
    let opt = |o: &Option<String>| match o {
        Some(s) => quote! { ::std::option::Option::Some(#s) },
        None => quote! { ::std::option::Option::None },
    };
    let tag = opt(&serde.tag);
    let content = opt(&serde.content);
    let untagged = serde.untagged;
    quote! {
        ::vld::__private::enum_schema(#tag, #content, #untagged, ::std::vec![ #( #entries ),* ])
    }
}

// ---------------------------------------------------------------------------
// Attributes
// ---------------------------------------------------------------------------

fn rename_names(meta: &syn::meta::ParseNestedMeta, what: &str) -> syn::Result<Names> {
    let (de, ser) = names(meta)?;
    for conv in de.iter().chain(ser.iter()) {
        if !RENAME_RULES.contains(&conv.as_str()) {
            return Err(meta.error(format!("unknown {what} rule `{conv}`")));
        }
    }
    Ok((de, ser))
}

fn parse_serde_enum_attrs(attrs: &[syn::Attribute]) -> syn::Result<SerdeEnum> {
    let mut out = SerdeEnum::default();
    for attr in serde_attrs(attrs) {
        attr.parse_nested_meta(|meta| {
            let p = &meta.path;
            if p.is_ident("tag") {
                out.tag = Some(meta.value()?.parse::<LitStr>()?.value());
            } else if p.is_ident("content") {
                out.content = Some(meta.value()?.parse::<LitStr>()?.value());
            } else if p.is_ident("untagged") {
                out.untagged = true;
            } else if p.is_ident("rename_all") {
                let (de, ser) = rename_names(&meta, "rename_all")?;
                out.rename_all = (
                    de.or(out.rename_all.0.take()),
                    ser.or(out.rename_all.1.take()),
                );
            } else if p.is_ident("rename_all_fields") {
                let (de, ser) = rename_names(&meta, "rename_all_fields")?;
                out.rename_all_fields = (
                    de.or(out.rename_all_fields.0.take()),
                    ser.or(out.rename_all_fields.1.take()),
                );
            } else {
                skip_meta(&meta)?;
            }
            Ok(())
        })?;
    }
    Ok(out)
}

fn parse_serde_variant_attrs(attrs: &[syn::Attribute]) -> syn::Result<SerdeVariant> {
    let mut out = SerdeVariant::default();
    for attr in serde_attrs(attrs) {
        attr.parse_nested_meta(|meta| {
            let p = &meta.path;
            if p.is_ident("rename") {
                let (de, ser) = names(&meta)?;
                out.rename = (de.or(out.rename.0.take()), ser.or(out.rename.1.take()));
            } else if p.is_ident("alias") {
                out.aliases.push(meta.value()?.parse::<LitStr>()?.value());
            } else if p.is_ident("skip") || p.is_ident("skip_deserializing") {
                out.skip = true;
            } else if p.is_ident("other") {
                out.other = true;
            } else if p.is_ident("rename_all") {
                let (de, ser) = rename_names(&meta, "rename_all")?;
                out.rename_all = (
                    de.or(out.rename_all.0.take()),
                    ser.or(out.rename_all.1.take()),
                );
            } else if p.is_ident("untagged") || p.is_ident("with") || p.is_ident("deserialize_with")
            {
                return Err(meta.error(format!(
                    "vld: #[serde({})] on a variant is not supported by #[derive(Validate)]",
                    p.get_ident().map(|i| i.to_string()).unwrap_or_default()
                )));
            } else {
                skip_meta(&meta)?;
            }
            Ok(())
        })?;
    }
    Ok(out)
}

/// serde's `rename_all` for variant names (written in PascalCase).
fn rename_variant(name: &str, convention: &str) -> String {
    let snake = || {
        let mut out = String::new();
        for (i, ch) in name.char_indices() {
            if i > 0 && ch.is_uppercase() {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
        }
        out
    };
    match convention {
        "lowercase" => name.to_ascii_lowercase(),
        "UPPERCASE" => name.to_ascii_uppercase(),
        "PascalCase" => name.to_string(),
        "camelCase" => {
            let mut chars = name.chars();
            match chars.next() {
                Some(first) => first.to_lowercase().chain(chars).collect(),
                None => String::new(),
            }
        }
        "snake_case" => snake(),
        "SCREAMING_SNAKE_CASE" => snake().to_ascii_uppercase(),
        "kebab-case" => snake().replace('_', "-"),
        "SCREAMING-KEBAB-CASE" => snake().replace('_', "-").to_ascii_uppercase(),
        _ => name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::rename_variant;

    /// The compile error `#[derive(Validate)]` reports for `src`.
    fn derive_error(src: &str) -> String {
        let input: syn::DeriveInput = syn::parse_str(src).unwrap();
        crate::expand(&input).unwrap_err().to_string()
    }

    #[test]
    fn unsupported_shapes_are_compile_errors() {
        let e = derive_error(r#"#[serde(tag = "t")] enum E { A(#[vld(x)] i64, #[vld(y)] i64) }"#);
        assert!(e.contains("cannot be internally tagged"), "{e}");
        let e = derive_error(r#"#[serde(tag = "t")] enum E { #[serde(other)] A(i64) }"#);
        assert!(e.contains("must be on a unit variant"), "{e}");
        let e = derive_error(r#"enum E { #[serde(other)] A }"#);
        assert!(e.contains("internally or adjacently tagged"), "{e}");
        let e = derive_error(r#"#[serde(content = "c")] enum E { A }"#);
        assert!(e.contains("needs #[serde(tag"), "{e}");
        let e = derive_error(r#"enum E { #[serde(untagged)] A }"#);
        assert!(e.contains("not supported"), "{e}");
        let e = derive_error(r#"enum E { A(i64, #[vld(x)] i64) }"#);
        assert!(e.contains("missing a #[vld"), "{e}");
        let e = derive_error(r#"enum E { A { x: i64 } }"#);
        assert!(e.contains("missing a #[vld"), "{e}");
        let e = derive_error(r#"#[vld(lenient)] enum E { A }"#);
        assert!(e.contains("struct-only"), "{e}");
        let e = derive_error(r#"union U { a: u8 }"#);
        assert!(e.contains("structs and enums"), "{e}");
    }

    #[test]
    fn variant_rename_rules_match_serde() {
        assert_eq!(rename_variant("CreateUser", "snake_case"), "create_user");
        assert_eq!(rename_variant("CreateUser", "kebab-case"), "create-user");
        assert_eq!(rename_variant("CreateUser", "camelCase"), "createUser");
        assert_eq!(rename_variant("CreateUser", "lowercase"), "createuser");
        assert_eq!(rename_variant("CreateUser", "UPPERCASE"), "CREATEUSER");
        assert_eq!(
            rename_variant("CreateUser", "SCREAMING_SNAKE_CASE"),
            "CREATE_USER"
        );
        assert_eq!(
            rename_variant("CreateUser", "SCREAMING-KEBAB-CASE"),
            "CREATE-USER"
        );
        assert_eq!(rename_variant("CreateUser", "PascalCase"), "CreateUser");
    }
}
