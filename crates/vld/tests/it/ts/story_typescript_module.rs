//! Stories: a project generates a TypeScript module from its vld types —
//! `impl_to_zod!` / `impl_to_zod4!` / `impl_to_valibot!` / `impl_to_ts_type!`
//! / `impl_to_openapi!` on `schema!` structs with nested `$ref`s, renamed
//! fields, custom export names and a serde-tagged enum; the module text is
//! snapshotted per generator.

use serde_json::json;
use vld::schema::VldSchema as _;
use vld::ts::{ToOpenApi, ToRefs, ToTsType, ToValibot, ToZod, ToZod4};

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

vld::schema! {
    #[derive(Debug)]
    pub struct TsCustomNamed {
        pub name: String => vld::string().min(1),
    }
}

vld::schema! {
    #[derive(Debug)]
    pub struct TsProfile {
        pub email: String => vld::string().email(),
        pub nickname: Option<String> => vld::string().min(2).optional(),
        pub role: String => vld::enumeration(&["admin", "user"]),
        pub address: TsAddress => vld::nested!(TsAddress),
    }
}

vld::schema! {
    pub struct Ordered {
        pub kind: String => vld::string(),
        pub zeta: String => vld::string(),
        pub alpha: f64 => vld::number(),
    }
}

vld::ts::impl_to_zod!(TsAddress);
vld::ts::impl_to_zod!(TsOrder);
vld::ts::impl_to_zod!(TsRenamed);
vld::ts::impl_to_zod!(TsCustomNamed, "RequestBody");
vld::ts::impl_to_zod!(Ordered);
vld::ts::impl_to_valibot!(TsAddress);
vld::ts::impl_to_valibot!(TsOrder);
vld::ts::impl_to_valibot!(TsRenamed);
vld::ts::impl_to_valibot!(TsCustomNamed, "RequestBody");
vld::ts::impl_to_zod4!(TsAddress);
vld::ts::impl_to_zod4!(TsOrder);
vld::ts::impl_to_zod4!(TsProfile);
vld::ts::impl_to_zod4!(TsCustomNamed, "RequestBody");
vld::ts::impl_to_ts_type!(TsAddress);
vld::ts::impl_to_ts_type!(TsOrder);
vld::ts::impl_to_ts_type!(TsProfile);
vld::ts::impl_to_ts_type!(TsCustomNamed, "RequestBody");
vld::ts::impl_to_openapi!(TsOrder);
vld::ts::impl_to_openapi!(TsRenamed);
vld::ts::impl_to_openapi!(TsCustomNamed, "RequestBody");

/// Given `schema!` structs with a nested type, renamed fields and a custom
/// export name, when a project writes one TypeScript module per generator,
/// then each type is exported under its name, refs point at the nested
/// `<Name>Schema`, and the modules match the snapshots.
#[test]
fn a_module_per_generator() {
    // Exports: names, schema constants, inferred types.
    assert_eq!(
        TsOrder::to_zod_export(),
        format!(
            "export const TsOrderSchema = {};\nexport type TsOrder = z.infer<typeof TsOrderSchema>;\n",
            TsOrder::to_zod()
        )
    );
    assert_eq!(
        TsCustomNamed::to_valibot_export(),
        format!(
            "export const RequestBodySchema = {};\nexport type RequestBody = v.InferOutput<typeof RequestBodySchema>;\n",
            TsCustomNamed::to_valibot()
        )
    );
    assert_eq!(
        TsProfile::to_zod4_export(),
        vld::ts::zod4_export("TsProfile", &TsProfile::to_zod4())
    );
    assert_eq!(
        TsCustomNamed::to_ts_type_export(),
        format!(
            "export type RequestBody = {};\n",
            TsCustomNamed::to_ts_type()
        )
    );
    assert_eq!(TsAddress::zod_name(), "TsAddress");
    assert_eq!(TsCustomNamed::zod_name(), "RequestBody");
    assert_eq!(TsCustomNamed::zod4_name(), "RequestBody");
    assert_eq!(TsCustomNamed::valibot_name(), "RequestBody");
    assert_eq!(TsCustomNamed::ts_type_name(), "RequestBody");
    assert_eq!(TsOrder::valibot_name(), "TsOrder");
    assert_eq!(TsOrder::zod4_name(), "TsOrder");
    assert_eq!(TsOrder::ts_type_name(), "TsOrder");
    assert_eq!(
        vld::ts::zod_export("Email", "z.string()"),
        "export const EmailSchema = z.string();\nexport type Email = z.infer<typeof EmailSchema>;\n"
    );
    assert_eq!(
        vld::ts::ts_type_export("Tags", "string[]"),
        "export type Tags = string[];\n"
    );
    assert_eq!(
        vld::ts::valibot_export("A", "v.string()"),
        "export const ASchema = v.string();\nexport type A = v.InferOutput<typeof ASchema>;\n"
    );

    // Nested refs and wire names.
    assert!(TsOrder::to_zod().contains("shipping: z.lazy(() => TsAddressSchema)"));
    assert!(TsOrder::to_valibot().contains("shipping: v.lazy(() => TsAddressSchema)"));
    assert!(TsOrder::to_zod4().contains("shipping: z.lazy(() => TsAddressSchema)"));
    assert!(TsOrder::to_ts_type().contains("shipping: TsAddress;"));
    assert!(TsRenamed::to_zod().contains("firstName: z.string().min(1)"));
    assert!(TsRenamed::to_valibot().contains("isActive: v.boolean()"));

    let zod = [
        TsOrder::to_zod(),
        TsRenamed::to_zod(),
        TsCustomNamed::to_zod(),
        TsOrder::to_zod_export(),
        TsCustomNamed::to_zod_export(),
        vld::ts::to_zod(
            &vld::array(vld::string().min(1))
                .max_len(10)
                .describe("Tags"),
        ),
    ]
    .join("\n\n");
    insta::assert_snapshot!("zod", zod);

    let valibot = [
        TsOrder::to_valibot(),
        TsRenamed::to_valibot(),
        TsCustomNamed::to_valibot(),
        TsCustomNamed::to_valibot_export(),
        vld::ts::to_valibot(&vld::string().min(2).email()),
    ]
    .join("\n\n");
    insta::assert_snapshot!("valibot", valibot);

    let zod4 = [
        TsOrder::to_zod4(),
        TsProfile::to_zod4_export(),
        TsCustomNamed::to_zod4_export(),
        vld::ts::to_zod4(&vld::string().uuid()),
        vld::ts::to_zod4(&vld::string().iso_datetime()),
        vld::ts::to_zod4(&vld::string().iso_date()),
        vld::ts::to_zod4(&vld::string().iso_time()),
        vld::ts::to_zod4(&vld::string().ipv4()),
        vld::ts::to_zod4(&vld::string().url()),
        vld::ts::to_zod4(&vld::number().int().min(1)),
        vld::ts::to_zod4(&vld::array(vld::string()).unique()),
    ]
    .join("\n\n");
    insta::assert_snapshot!("zod4", zod4);

    let ts = [
        TsAddress::to_ts_type_export(),
        TsProfile::to_ts_type_export(),
        TsCustomNamed::to_ts_type_export(),
    ]
    .join("\n");
    insta::assert_snapshot!("ts_type", ts);
}

/// Given the same types, when a project asks for plain JSON Schema, then it
/// gets vld's `json_schema()` and the `$ref`s it points at (sorted, unique).
#[test]
fn openapi_and_refs() {
    let doc = TsOrder::to_openapi();
    assert_eq!(doc, TsOrder::json_schema());
    assert_eq!(
        doc["properties"]["shipping"]["$ref"],
        "#/components/schemas/TsAddress"
    );
    assert_eq!(
        vld::ts::openapi_refs(&doc),
        ["#/components/schemas/TsAddress"]
    );
    assert_eq!(TsOrder::to_refs(), ["#/components/schemas/TsAddress"]);
    assert!(TsRenamed::to_refs().is_empty());
    assert!(TsCustomNamed::to_refs().is_empty());
    assert_eq!(
        vld::ts::openapi_refs(&json!({
            "anyOf": [{"$ref": "#/b"}, {"items": {"$ref": "#/a"}}, {"$ref": "#/b"}, {"$ref": 1}]
        })),
        ["#/a", "#/b"]
    );
    let schema = vld::ts::to_openapi(&vld::string().email());
    assert_eq!(
        (schema["type"].clone(), schema["format"].clone()),
        (json!("string"), json!("email"))
    );

    insta::with_settings!({ sort_maps => true }, {
        insta::assert_json_snapshot!(
            "openapi",
            json!({
                "order": TsOrder::to_openapi(),
                "renamed": TsRenamed::to_openapi(),
                "custom_named": TsCustomNamed::to_openapi(),
            })
        );
    });
}

#[derive(serde::Serialize, serde::Deserialize, vld::Validate)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[allow(dead_code)]
pub enum Shape {
    Empty,
    Circle {
        #[vld(vld::number().min(0.0))]
        radius: f64,
    },
    Rect {
        #[vld(vld::number())]
        w: f64,
        #[vld(vld::number())]
        h: f64,
    },
}
vld::ts::impl_to_zod!(Shape);
vld::ts::impl_to_valibot!(Shape);
vld::ts::impl_to_ts_type!(Shape);
vld::ts::impl_to_zod4!(Shape);

/// Given a serde-tagged enum and a struct whose fields are not sorted, when
/// code is generated, then the enum is a discriminated union on its tag with
/// the tag first in each variant, and every object keeps declaration order.
#[test]
fn tagged_enums_and_field_order() {
    let z = Shape::to_zod();
    assert!(
        z.starts_with(r#"z.discriminatedUnion("kind", [z.object({"#),
        "{z}"
    );
    assert!(
        z.contains("kind: z.literal(\"rect\"),\n  w: z.number(),\n  h: z.number()"),
        "{z}"
    );
    let v = Shape::to_valibot();
    assert!(v.starts_with(r#"v.variant("kind", [v.object({"#), "{v}");
    let t = Shape::to_ts_type();
    assert!(t.contains("kind: \"empty\";"), "{t}");
    assert!(t.contains(" | "), "{t}");

    let z = Ordered::to_zod();
    let pos = |k: &str| z.find(k).unwrap_or_else(|| panic!("{k} in {z}"));
    assert!(
        pos("kind") < pos("zeta") && pos("zeta") < pos("alpha"),
        "{z}"
    );
}
