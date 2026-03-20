use vld::schema::VldSchema;
use vld_ts::{ToOpenApi, ToRefs, ToValibot, ToZod};

#[test]
fn vld_schema_string_with_constraints() {
    let zod = vld_ts::to_zod(&vld::string().min(2).max(50).email());
    assert_eq!(zod, "z.string().min(2).max(50).email()");
}

#[test]
fn vld_schema_integer_constraints() {
    let zod = vld_ts::to_zod(&vld::number().int().min(0).max(100));
    assert_eq!(zod, "z.number().int().min(0).max(100)");
}

#[test]
fn vld_schema_nullable() {
    let zod = vld_ts::to_zod(&vld::string().nullable());
    assert_eq!(zod, "z.string().nullable()");
}

#[test]
fn vld_schema_union() {
    let zod = vld_ts::to_zod(&vld::union(vld::string(), vld::number()));
    assert_eq!(zod, "z.union([z.string(), z.number()])");
}

#[test]
fn vld_schema_array_and_description() {
    let zod = vld_ts::to_zod(
        &vld::array(vld::string().min(1))
            .max_len(10)
            .describe("Tags"),
    );
    assert!(zod.contains("z.array(z.string().min(1)).max(10)"));
    assert!(zod.contains(".describe(\"Tags\")"));
}

vld::schema! {
    #[derive(Debug)]
    pub struct TsAddress {
        pub city: String => vld::string().min(1),
        pub zip: String => vld::string().min(5).max(10),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct TsOrder {
        pub id: String => vld::string().min(1),
        pub shipping: TsAddress => vld::nested!(TsAddress),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct TsRenamed {
        pub first_name: String as "firstName" => vld::string().min(1),
        pub is_active: bool as "isActive" => vld::boolean(),
    }
}

vld_ts::impl_to_zod!(TsOrder);
vld_ts::impl_to_zod!(TsRenamed);
vld_ts::impl_to_openapi!(TsOrder);
vld_ts::impl_to_openapi!(TsRenamed);
vld_ts::impl_to_valibot!(TsOrder);
vld_ts::impl_to_valibot!(TsRenamed);

vld::schema! {
    #[derive(Debug)]
    pub struct TsCustomNamed {
        pub name: String => vld::string().min(1),
    }
}

vld_ts::impl_to_zod!(TsCustomNamed, "RequestBody");
vld_ts::impl_to_openapi!(TsCustomNamed, "RequestBody");
vld_ts::impl_to_valibot!(TsCustomNamed, "RequestBody");

#[test]
fn type_to_zod_single_with_nested() {
    let ts = TsOrder::to_zod();
    assert!(ts.starts_with("z.object("));
    assert!(!ts.contains("import { z } from \"zod\""));
    assert!(!ts.contains("export const TsOrderSchema"));
    assert!(!ts.contains("export const TsAddressSchema"));
    assert!(ts.contains("shipping: z.lazy(() => TsAddressSchema)"));
}

#[test]
fn type_to_zod_respects_renamed_fields() {
    let ts = TsRenamed::to_zod();
    assert!(ts.contains("firstName: z.string().min(1)"));
    assert!(ts.contains("isActive: z.boolean()"));
    assert!(!ts.contains("first_name:"));
    assert!(!ts.contains("is_active:"));
}

#[test]
fn impl_to_zod_custom_name() {
    let ts = TsCustomNamed::to_zod();
    assert!(ts.starts_with("z.object("));
    assert!(ts.contains("name: z.string().min(1)"));
    assert!(!ts.contains("RequestBodySchema"));
}

#[test]
fn to_valibot_from_schema_instance() {
    let ts = vld_ts::to_valibot(&vld::string().min(2).email());
    assert!(ts.contains("v.string()"));
    assert!(ts.contains("v.minLength(2)"));
    assert!(ts.contains("v.email()"));
}

#[test]
fn type_to_valibot_single_with_nested() {
    let ts = TsOrder::to_valibot();
    assert!(ts.starts_with("v.object"));
    assert!(!ts.contains("import * as v from \"valibot\""));
    assert!(!ts.contains("export const TsOrderSchema"));
    assert!(!ts.contains("export const TsAddressSchema"));
    assert!(ts.contains("shipping: v.lazy(() => TsAddressSchema)"));
}

#[test]
fn impl_to_valibot_custom_name() {
    let ts = TsCustomNamed::to_valibot();
    assert!(ts.starts_with("v.object"));
    assert!(ts.contains("name: v.pipe(v.string(), v.minLength(1))"));
    assert!(!ts.contains("RequestBodySchema"));
}

#[test]
fn to_openapi_from_schema_instance() {
    let schema = vld_ts::to_openapi(&vld::string().email());
    assert_eq!(schema["type"], "string");
    assert_eq!(schema["format"], "email");
}

#[test]
fn type_to_openapi_single_with_nested() {
    let doc = TsOrder::to_openapi();
    assert_eq!(doc["type"], "object");
    assert_eq!(
        doc["properties"]["shipping"]["$ref"],
        "#/components/schemas/TsAddress"
    );
    let refs = vld_ts::openapi_refs(&doc);
    assert!(refs.contains(&"#/components/schemas/TsAddress".to_string()));
    let refs2 = TsOrder::to_refs();
    assert!(refs2.contains(&"#/components/schemas/TsAddress".to_string()));
}

#[test]
fn type_to_openapi_custom_name() {
    let doc = TsCustomNamed::to_openapi();
    assert_eq!(doc["type"], "object");
    assert!(doc["properties"]["name"].is_object());
}
