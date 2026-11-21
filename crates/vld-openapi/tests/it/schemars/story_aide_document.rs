//! Story: an aide app documents its vld request types. aide asks schemars for
//! each type (`impl_json_schema!` of the schemars module); nested types land under
//! aide's `#/components/schemas/`, renamed fields keep their wire names, the
//! required sets follow vld, and OpenAPI 3.0 generators get 3.0 schemas.

use super::bridge::schemars::{self, JsonSchema};
use super::bridge::{impl_json_schema, vld_to_schemars};
use serde_json::{Value, json};
use vld::openapi::JsonSchema as _;

vld::schema! {
    #[derive(Debug)]
    pub struct AideAddress {
        pub city: String => vld::string().min(1),
        pub zip: String => vld::string().min(5).max(10),
    }
}
impl_json_schema!(AideAddress);

vld::schema! {
    #[derive(Debug)]
    pub struct AideOrder {
        pub name: String => vld::string().min(1),
        pub address: AideAddress => vld::nested!(AideAddress),
    }
}
impl_json_schema!(AideOrder);

vld::schema! {
    #[derive(Debug)]
    pub struct TestUser {
        pub name: String => vld::string().min(2).max(50),
        pub email: String => vld::string().email(),
        pub age: Option<i64> => vld::number().int().gte(0).optional(),
    }
}
impl_json_schema!(TestUser);

vld::schema! {
    #[derive(Debug)]
    pub struct ReqBody {
        pub x: String => vld::string().min(1),
    }
}
impl_json_schema!(ReqBody, "CreateRequest");

#[derive(Debug, vld::Validate)]
#[allow(dead_code)]
struct DeriveUser {
    #[vld(vld::string().min(2).max(50))]
    name: String,
    #[vld(vld::string().email())]
    email: String,
    #[vld(vld::number().int().gte(0).optional())]
    age: Option<i64>,
}
impl_json_schema!(DeriveUser);

#[derive(Debug, serde::Deserialize, vld::Validate)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct DeriveRenamedRequest {
    #[vld(vld::string().min(1).max(255))]
    first_name: String,
    #[vld(vld::string().email())]
    email_address: String,
    #[vld(vld::number().int().non_negative().min(1).max(9999))]
    street_number: i64,
    #[vld(vld::boolean())]
    is_active: bool,
}
impl_json_schema!(DeriveRenamedRequest);

vld::schema! {
    #[derive(Debug)]
    pub struct AideCreateTicket {
        pub title: String => vld::string()
            .min(3)
            .max(120)
            .describe("ticket title")
            .message("bad title"),
        pub body: String => vld::string().min(1),
        pub priority: String => vld::string().with_default("normal".into()),
        pub labels: Option<Vec<String>> => vld::array(vld::string().min(1)).optional(),
        pub due_at: Option<String> => vld::string().nullish().describe("ISO date"),
        pub reporter: Option<String> => vld::string()
            .email()
            .nullable()
            .describe("reporter email"),
        pub meta: Option<String> => vld::string()
            .optional()
            .describe("freeform")
            .catch(None),
    }
}
impl_json_schema!(AideCreateTicket);

#[derive(Debug, vld::Validate)]
#[allow(dead_code)]
struct AideDerivePatch {
    #[vld(vld::string().min(3).describe("title"))]
    title: String,
    #[vld(vld::string().optional().describe("body"))]
    body: Option<String>,
    #[vld(vld::string().with_default("normal".into()))]
    priority: String,
    #[vld(vld::string().nullish())]
    assignee: Option<String>,
    #[vld(vld::string().nullable().describe("cursor"))]
    cursor: Option<String>,
}
impl_json_schema!(AideDerivePatch);

vld::schema! {
    #[derive(Debug)]
    pub struct Oas3 {
        pub pair: (String, u16) => (vld::string(), vld::u16()),
        pub nick: Option<String> => vld::string().nullable(),
    }
}
impl_json_schema!(Oas3);

/// `json_schema` through a fresh default generator.
fn schema_of<T: JsonSchema>() -> Value {
    let mut generator = schemars::SchemaGenerator::default();
    serde_json::to_value(T::json_schema(&mut generator)).unwrap()
}

fn required(schema: &Value) -> Vec<&str> {
    schema["required"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

/// Given `schema!` and derive types, when aide asks schemars for them, then
/// they carry their names, ids, the vld rules and wire names.
#[test]
fn request_types_are_schemars_types() {
    assert_eq!(TestUser::schema_name(), "TestUser");
    assert_eq!(ReqBody::schema_name(), "CreateRequest");
    assert_eq!(DeriveUser::schema_name(), "DeriveUser");
    assert_eq!(
        TestUser::schema_id(),
        format!("{}::TestUser", module_path!())
    );

    for user in [schema_of::<TestUser>(), schema_of::<DeriveUser>()] {
        assert_eq!(user["type"], "object");
        let p = &user["properties"];
        assert_eq!(
            (
                p["name"]["type"].clone(),
                p["name"]["minLength"].clone(),
                p["name"]["maxLength"].clone()
            ),
            (json!("string"), json!(2), json!(50))
        );
        assert_eq!(p["email"]["format"], "email");
        assert!(p["age"].is_object());
        assert_eq!(required(&user), ["name", "email"]);
    }
    assert_eq!(schema_of::<ReqBody>()["type"], "object");

    let renamed = schema_of::<DeriveRenamedRequest>();
    let mut keys: Vec<&String> = renamed["properties"].as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(
        keys,
        ["emailAddress", "firstName", "isActive", "streetNumber"]
    );

    // The plain conversion of the full struct keeps the required set.
    let converted = serde_json::to_value(vld_to_schemars(&TestUser::json_schema())).unwrap();
    assert_eq!(converted["type"], "object");
    assert_eq!(required(&converted), ["name", "email"]);
    let number =
        serde_json::to_value(vld_to_schemars(&vld::number().min(0.0).json_schema())).unwrap();
    assert_eq!(number["type"], "number");
}

/// Given optional, nullable, nullish, defaulted and caught fields, when aide
/// documents a `schema!` and a derive type, then only plain and nullable
/// fields are required, and the documents match the snapshot (schemars 0.9
/// and 1.x agree).
#[test]
fn the_required_matrix_follows_vld() {
    let js = AideCreateTicket::json_schema();
    assert_eq!(required(&js), ["title", "body", "reporter"]);
    assert_eq!(serde_json::to_value(vld_to_schemars(&js)).unwrap(), js);

    let generated = schema_of::<AideCreateTicket>();
    assert_eq!(required(&generated), ["title", "body", "reporter"]);
    let derive = schema_of::<AideDerivePatch>();
    assert_eq!(required(&derive), ["title", "cursor"]);
    assert_eq!(
        required(&AideDerivePatch::json_schema()),
        ["title", "cursor"]
    );

    // schemars 0.9 and 1.x share one file.
    insta::with_settings!({ sort_maps => true, prepend_module_to_snapshot => false }, {
        insta::assert_json_snapshot!(
            "it__schemars__story_aide_document__required_matrix",
            json!({"create_ticket": generated, "derive_patch": derive})
        );
    });
}

/// Given an order with a nested address, when aide (definitions under
/// `#/components/schemas/`) and a default generator document it, then the ref
/// follows each generator's path and the address is a registered definition;
/// grandchildren are registered transitively (issue #5).
#[test]
fn nested_types_become_components() {
    let mut aide = schemars::generate::SchemaSettings::draft07()
        .with(|s| s.definitions_path = "#/components/schemas/".into())
        .into_generator();
    let order = <AideOrder as JsonSchema>::json_schema(&mut aide);
    assert_eq!(
        order.get("properties").unwrap()["address"]["$ref"],
        "#/components/schemas/AideAddress"
    );
    let addr = &aide.definitions()["AideAddress"];
    assert_eq!(addr["type"], "object");
    assert!(addr["properties"]["city"].is_object());

    let mut generator = schemars::SchemaGenerator::default();
    let order = <AideOrder as JsonSchema>::json_schema(&mut generator);
    assert_eq!(
        order.get("properties").unwrap()["address"]["$ref"],
        "#/$defs/AideAddress"
    );
    assert!(generator.definitions().contains_key("AideAddress"));

    vld::schema! {
        #[derive(Debug)]
        pub struct AideBreak {
            pub label: String => vld::string().min(1),
        }
    }
    impl_json_schema!(AideBreak);
    vld::schema! {
        #[derive(Debug)]
        pub struct AideDay {
            pub breaks: Option<Vec<AideBreak>> =>
                vld::array(vld::nested!(AideBreak)).optional(),
        }
    }
    impl_json_schema!(AideDay);
    vld::schema! {
        #[derive(Debug)]
        pub struct AideSchedule {
            pub days: Vec<AideDay> => vld::array(vld::nested!(AideDay)),
        }
    }
    impl_json_schema!(AideSchedule);

    let mut generator = schemars::SchemaGenerator::default();
    let _ = <AideSchedule as JsonSchema>::json_schema(&mut generator);
    let mut defs: Vec<&String> = generator.definitions().keys().collect();
    defs.sort();
    assert_eq!(defs, ["AideBreak", "AideDay"]);
}

/// Given aide configured for OpenAPI 3.0, when it documents a tuple and a
/// nullable field, then the schema has no 2020-12 keywords: the tuple is
/// `items` with fixed length, the nullable field `nullable: true`.
#[test]
fn openapi3_settings_yield_openapi3_schemas() {
    fn keys(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(m) => {
                for (k, v) in m {
                    out.push(k.clone());
                    keys(v, out);
                }
            }
            Value::Array(a) => a.iter().for_each(|v| keys(v, out)),
            _ => {}
        }
    }
    let generator = schemars::generate::SchemaSettings::openapi3().into_generator();
    let json = serde_json::to_value(generator.into_root_schema_for::<Oas3>()).unwrap();
    let mut all = Vec::new();
    keys(&json, &mut all);
    for bad in ["prefixItems", "additionalItems"] {
        assert!(!all.iter().any(|k| k == bad), "{bad} in {json}");
    }
    let pair = &json["properties"]["pair"];
    assert_eq!(pair["type"], "array");
    assert!(pair["items"].is_object(), "{pair}");
    assert_eq!(
        (pair["minItems"].clone(), pair["maxItems"].clone()),
        (json!(2), json!(2))
    );
    let nick = &json["properties"]["nick"];
    assert_eq!(
        (nick["type"].clone(), nick["nullable"].clone()),
        (json!("string"), json!(true))
    );
    assert!(nick.get("enum").is_none(), "{nick}");
}
